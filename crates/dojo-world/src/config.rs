use anyhow::{anyhow, Result};
use scarb::core::Workspace;
use serde::{Deserialize, Serialize};
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};
use toml::Value;
use url::Url;

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug)]
pub enum DeserializationError {
    #[error("parsing field element")]
    ParsingFieldElement,
    #[error("parsing url")]
    ParsingUrl,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldConfig {
    pub address: Option<FieldElement>,
}

pub struct DeploymentConfig {
    pub rpc: Option<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployments {
    pub testnet: Option<Deployment>,
    pub mainnet: Option<Deployment>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub rpc: Option<String>,
}

fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<Value> {
    ws.current_package().ok()?.manifest.metadata.tool_metadata.as_ref()?.get("dojo").cloned()
}

impl WorldConfig {
    pub fn from_workspace(ws: &Workspace<'_>) -> Result<Self, DeserializationError> {
        let mut world_config = WorldConfig::default();

        if let Some(dojo_metadata) = dojo_metadata_from_workspace(ws) {
            if let Some(world_address) = dojo_metadata.get("world_address") {
                if let Some(world_address) = world_address.as_str() {
                    let world_address = FieldElement::from_hex_be(world_address)
                        .map_err(|_| DeserializationError::ParsingFieldElement)?;
                    world_config.address = Some(world_address);
                }
            }
        }

        Ok(world_config)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct EnvironmentConfig {
    pub rpc: Option<Url>,
    pub private_key: Option<FieldElement>,
    pub account_address: Option<FieldElement>,
    pub keystore_path: Option<String>,
    pub keystore_password: Option<String>,
}

impl EnvironmentConfig {
    pub fn from_workspace<T: AsRef<str>>(profile: T, ws: &Workspace<'_>) -> Result<Self> {
        let mut config = EnvironmentConfig::default();

        let mut env_metadata = dojo_metadata_from_workspace(ws)
            .and_then(|dojo_metadata| dojo_metadata.get("env").cloned());

        // If there is an environment-specific metadata, use that, otherwise use the
        // workspace's default environment metadata.
        env_metadata = env_metadata
            .as_ref()
            .and_then(|env_metadata| env_metadata.get(profile.as_ref()).cloned())
            .or(env_metadata);

        if let Some(env) = env_metadata {
            if let Some(rpc) = env.get("rpc_url").and_then(|v| v.as_str()) {
                let url = Url::parse(rpc).map_err(|_| DeserializationError::ParsingUrl)?;
                config.rpc = Some(url);
            }

            if let Some(private_key) = env
                .get("private_key")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_PRIVATE_KEY").ok())
            {
                let pk = FieldElement::from_hex_be(&private_key)
                    .map_err(|_| DeserializationError::ParsingFieldElement)?;
                config.private_key = Some(pk);
            }

            if let Some(path) = env
                .get("keystore_path")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_KEYSTORE_PATH").ok())
            {
                config.keystore_path = Some(path);
            }

            if let Some(password) = env
                .get("keystore_password")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_KEYSTORE_PASSWORD").ok())
            {
                config.keystore_password = Some(password);
            }

            if let Some(account_address) = env
                .get("account_address")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_ACCOUNT_ADDRESS").ok())
            {
                let address = FieldElement::from_hex_be(&account_address)
                    .map_err(|_| DeserializationError::ParsingFieldElement)?;
                config.account_address = Some(address);
            }
        }

        Ok(config)
    }

    pub fn signer(&self) -> Result<LocalWallet> {
        if let Some(private_key) = &self.private_key {
            Ok(LocalWallet::from_signing_key(SigningKey::from_secret_scalar(*private_key)))
        } else if let Some(keystore_path) = &self.keystore_path {
            let keystore_password = self
                .keystore_password
                .as_ref()
                .ok_or_else(|| anyhow!("Missing `keystore_password` in the environment config"))?;

            Ok(LocalWallet::from_signing_key(SigningKey::from_keystore(
                keystore_path,
                keystore_password,
            )?))
        } else {
            Err(anyhow!("Missing `private_key` or `keystore_path` in the environment config"))
        }
    }

    pub fn provider(&self) -> Result<JsonRpcClient<HttpTransport>> {
        let Some(url) = &self.rpc else {
            return Err(anyhow!("Missing `rpc_url` in the environment config"))
        };

        Ok(JsonRpcClient::new(HttpTransport::new(url.clone())))
    }

    pub fn account_address(&self) -> Result<FieldElement> {
        self.account_address.ok_or(anyhow!("Missing `account_address` in the environment config"))
    }

    pub async fn migrator(
        &self,
    ) -> Result<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>> {
        let signer = self.signer()?;
        let account_address = self.account_address()?;

        let provider = self.provider()?;
        let chain_id = provider.chain_id().await?;

        Ok(SingleOwnerAccount::new(provider, signer, account_address, chain_id))
    }
}
