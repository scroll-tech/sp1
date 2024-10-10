use std::{env, time::Duration};

use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use anyhow::{Context, Ok, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use reqwest_middleware::ClientWithMiddleware as HttpClientWithMiddleware;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sp1_core_machine::io::SP1Stdin;
use sp1_prover::SP1VerifyingKey;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::OnceCell;
use tokio::try_join;
use tonic::transport::Channel;

use crate::network_v2::proto::artifact::{
    artifact_store_client::ArtifactStoreClient, CreateArtifactRequest,
};
use crate::network_v2::proto::network::{
    prover_network_client::ProverNetworkClient, GetFilteredProofRequestsRequest,
    GetFilteredProofRequestsResponse, GetNonceRequest, GetProofRequestStatusRequest,
    GetProofRequestStatusResponse, ProofMode, ProofStatus, ProofStrategy, RequestProofRequest,
    RequestProofRequestBody, RequestProofResponse,
};
use crate::network_v2::Signable;

/// The default RPC endpoint for the Succinct prover network.
pub const DEFAULT_PROVER_NETWORK_RPC: &str = "http://127.0.0.1:50051";

pub struct NetworkClient {
    signer: PrivateKeySigner,
    http: HttpClientWithMiddleware,
    s3: OnceCell<S3Client>,
}

impl NetworkClient {
    /// Create a new network client with the given private key.
    pub fn new(private_key: &str) -> Self {
        let signer = PrivateKeySigner::from_str(private_key).unwrap();

        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .pool_idle_timeout(Duration::from_secs(240))
            .build()
            .unwrap();

        Self { signer, http: http_client.into(), s3: OnceCell::new() }
    }

    /// Returns the currently configured RPC endpoint for the Succinct prover network.
    pub fn rpc_url() -> String {
        env::var("PROVER_NETWORK_RPC").unwrap_or_else(|_| DEFAULT_PROVER_NETWORK_RPC.to_string())
    }

    /// Get a connected RPC client.
    async fn get_rpc(&self) -> Result<ProverNetworkClient<Channel>> {
        let rpc_url = Self::rpc_url();
        let channel = Channel::from_shared(rpc_url)?.connect().await?;
        Ok(ProverNetworkClient::new(channel.clone()))
    }

    /// Get a connected artifact store client.
    async fn get_store(&self) -> Result<ArtifactStoreClient<Channel>> {
        let rpc_url = Self::rpc_url();
        let channel = Channel::from_shared(rpc_url)?.connect().await?;
        Ok(ArtifactStoreClient::new(channel.clone()))
    }

    /// Get the S3 client.
    async fn get_s3_client(&self) -> &S3Client {
        self.s3
            .get_or_init(|| async {
                let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
                S3Client::new(&config)
            })
            .await
    }

    /// Get the latest nonce for this account's address.
    pub async fn get_nonce(&self) -> Result<u64> {
        let mut rpc = self.get_rpc().await?;
        let res =
            rpc.get_nonce(GetNonceRequest { address: self.signer.address().to_vec() }).await?;
        Ok(res.into_inner().nonce)
    }

    /// Get the status of a given proof. If the status is Fulfilled, the proof is also returned.
    pub async fn get_proof_request_status<P: DeserializeOwned>(
        &self,
        request_id: &[u8],
    ) -> Result<(GetProofRequestStatusResponse, Option<P>)> {
        let mut rpc = self.get_rpc().await?;
        let res = rpc
            .get_proof_request_status(GetProofRequestStatusRequest {
                request_id: request_id.to_vec(),
            })
            .await?
            .into_inner();
        let status = ProofStatus::try_from(res.status)?;
        let proof = match status {
            ProofStatus::Fulfilled => {
                log::info!("Proof request fulfilled");
                let proof_uri = res
                    .proof_uri
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No proof URL provided"))?;
                let proof_bytes = self.download_artifact(proof_uri).await?;
                Some(bincode::deserialize(&proof_bytes).context("Failed to deserialize proof")?)
            }
            _ => None,
        };

        Ok((res, proof))
    }

    /// Get all the proof requests for a given status. Also filter by version if provided.
    pub async fn get_filtered_proof_requests(
        &self,
        status: ProofStatus,
        version: Option<&str>,
    ) -> Result<GetFilteredProofRequestsResponse> {
        let mut rpc = self.get_rpc().await?;
        let res = rpc
            .get_filtered_proof_requests(GetFilteredProofRequestsRequest {
                status: status.into(),
                version: version.map(|v| v.to_string()).unwrap_or_default(),
            })
            .await?
            .into_inner();

        Ok(res)
    }

    /// Creates a proof request with the given ELF and stdin.
    #[allow(clippy::too_many_arguments)]
    pub async fn request_proof(
        &self,
        elf: &[u8],
        stdin: &SP1Stdin,
        vk: &SP1VerifyingKey,
        mode: ProofMode,
        version: &str,
        strategy: ProofStrategy,
        timeout_secs: u64,
        cycle_limit: u64,
    ) -> Result<RequestProofResponse> {
        // Calculate the deadline.
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Invalid start time");
        let deadline = since_the_epoch.as_secs() + timeout_secs;

        // Create the program and stdin artifacts.
        let mut store = self.get_store().await?;
        let mut store_clone = store.clone();
        let program_promise = self.create_artifact_with_content(&mut store, &elf);
        let stdin_promise = self.create_artifact_with_content(&mut store_clone, &stdin);
        let (program_uri, stdin_uri) = try_join!(program_promise, stdin_promise)?;

        // Serialize the vkey.
        let vkey = bincode::serialize(&vk)?;

        // Send the request.
        let mut rpc = self.get_rpc().await?;
        let nonce = self.get_nonce().await?;
        let request_body = RequestProofRequestBody {
            nonce,
            version: format!("sp1-{}", version),
            vkey,
            mode: mode.into(),
            strategy: strategy.into(),
            program_uri,
            stdin_uri,
            deadline,
            cycle_limit,
        };
        let request_response = rpc
            .request_proof(RequestProofRequest {
                signature: request_body.sign(&self.signer).into(),
                body: Some(request_body),
            })
            .await?
            .into_inner();

        Ok(request_response)
    }

    /// Uses the artifact store to to create an artifact, upload the content, and return the URI.
    async fn create_artifact_with_content<T: Serialize>(
        &self,
        store: &mut ArtifactStoreClient<Channel>,
        item: &T,
    ) -> Result<String> {
        let signature = self.signer.sign_message_sync("create_artifact".as_bytes())?;
        let request = CreateArtifactRequest { signature: signature.as_bytes().to_vec() };
        let response = store.create_artifact(request).await?.into_inner();

        let presigned_url = response.artifact_presigned_url;
        let uri = response.artifact_uri;

        let response =
            self.http.put(&presigned_url).body(bincode::serialize::<T>(item)?).send().await?;

        assert!(response.status().is_success());

        Ok(uri)
    }

    /// Download an artifact from S3.
    async fn download_artifact(&self, uri: &str) -> Result<Vec<u8>> {
        let s3_client = self.get_s3_client().await;
        let uri = uri.strip_prefix("s3://").context("Invalid S3 URI")?;
        let (bucket, key) = uri.split_once('/').context("Invalid S3 URI format")?;

        let resp = s3_client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .context("Failed to get object from S3")?;

        let data = resp.body.collect().await.context("Failed to read S3 object body")?;
        Ok(data.into_bytes().to_vec())
    }
}
