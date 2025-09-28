use bundler_config::SigningConfig;
use bundler_types::{BundlerError, BundlerResult, SignerConfig, SignerType};
use solana_sdk::{
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
};
use std::{collections::HashMap, sync::Arc};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};
use bs58;

/// Enum for different key provider types
#[derive(Debug)]
pub enum KeyProvider {
    File(FileKeyProvider),
    Env(EnvKeyProvider),
    Kms(KmsKeyProvider),
}

impl KeyProvider {
    /// Get the public key for this signer
    pub async fn public_key(&self) -> BundlerResult<Pubkey> {
        match self {
            KeyProvider::File(provider) => provider.public_key().await,
            KeyProvider::Env(provider) => provider.public_key().await,
            KeyProvider::Kms(provider) => provider.public_key().await,
        }
    }
    
    /// Sign a message with this key
    pub async fn sign(&self, message: &[u8]) -> BundlerResult<Signature> {
        match self {
            KeyProvider::File(provider) => provider.sign(message).await,
            KeyProvider::Env(provider) => provider.sign(message).await,
            KeyProvider::Kms(provider) => provider.sign(message).await,
        }
    }
    
    /// Check if this key provider is healthy
    pub async fn health_check(&self) -> BundlerResult<()> {
        match self {
            KeyProvider::File(provider) => provider.health_check().await,
            KeyProvider::Env(provider) => provider.health_check().await,
            KeyProvider::Kms(provider) => provider.health_check().await,
        }
    }
}

/// File-based keypair provider (for development only)
#[derive(Debug)]
pub struct FileKeyProvider {
    keypair: Keypair,
}

impl FileKeyProvider {
    pub fn new(path: &str) -> BundlerResult<Self> {
        let keypair_bytes = std::fs::read(path)
            .map_err(|e| BundlerError::Signing(format!("Failed to read keypair file {}: {}", path, e)))?;
        
        let keypair = if keypair_bytes.len() == 64 {
            // Raw 64-byte keypair
            Keypair::from_bytes(&keypair_bytes)
                .map_err(|e| BundlerError::Signing(format!("Invalid keypair format: {}", e)))?
        } else {
            // Try to parse as JSON array
            let json_bytes: Vec<u8> = serde_json::from_slice(&keypair_bytes)
                .map_err(|e| BundlerError::Signing(format!("Failed to parse keypair JSON: {}", e)))?;
            
            if json_bytes.len() != 64 {
                return Err(BundlerError::Signing("Keypair must be 64 bytes".to_string()));
            }
            
            Keypair::from_bytes(&json_bytes)
                .map_err(|e| BundlerError::Signing(format!("Invalid keypair bytes: {}", e)))?
        };
        
        Ok(Self { keypair })
    }
    
    pub async fn public_key(&self) -> BundlerResult<Pubkey> {
        Ok(self.keypair.pubkey())
    }
    
    pub async fn sign(&self, message: &[u8]) -> BundlerResult<Signature> {
        Ok(self.keypair.sign_message(message))
    }
    
    pub async fn health_check(&self) -> BundlerResult<()> {
        // File-based keypairs are always "healthy" if they loaded successfully
        Ok(())
    }
}

/// Environment variable keypair provider
#[derive(Debug)]
pub struct EnvKeyProvider {
    keypair: Keypair,
}

impl EnvKeyProvider {
    pub fn new(var_name: &str) -> BundlerResult<Self> {
        let key_str = std::env::var(var_name)
            .map_err(|_| BundlerError::Signing(format!("Environment variable {} not found", var_name)))?;
        
        // Try to decode as base58 first (Solana CLI format)
        let keypair = if let Ok(bytes) = bs58::decode(&key_str).into_vec() {
            if bytes.len() == 64 {
                Keypair::from_bytes(&bytes)
                    .map_err(|e| BundlerError::Signing(format!("Invalid base58 keypair: {}", e)))?
            } else {
                return Err(BundlerError::Signing("Base58 keypair must decode to 64 bytes".to_string()));
            }
        } else {
            // Try to parse as JSON array
            let json_bytes: Vec<u8> = serde_json::from_str(&key_str)
                .map_err(|e| BundlerError::Signing(format!("Failed to parse keypair JSON: {}", e)))?;
            
            if json_bytes.len() != 64 {
                return Err(BundlerError::Signing("JSON keypair must be 64 bytes".to_string()));
            }
            
            Keypair::from_bytes(&json_bytes)
                .map_err(|e| BundlerError::Signing(format!("Invalid keypair bytes: {}", e)))?
        };
        
        Ok(Self { keypair })
    }
    
    pub async fn public_key(&self) -> BundlerResult<Pubkey> {
        Ok(self.keypair.pubkey())
    }
    
    pub async fn sign(&self, message: &[u8]) -> BundlerResult<Signature> {
        Ok(self.keypair.sign_message(message))
    }
    
    pub async fn health_check(&self) -> BundlerResult<()> {
        // Environment-based keypairs are always "healthy" if they loaded successfully
        Ok(())
    }
}

/// AWS KMS keypair provider (for production)
#[derive(Debug)]
pub struct KmsKeyProvider {
    key_id: String,
    region: String,
    public_key: Option<Pubkey>,
}

impl KmsKeyProvider {
    pub fn new(key_id: String, region: String) -> Self {
        Self {
            key_id,
            region,
            public_key: None,
        }
    }
    
    pub async fn public_key(&self) -> BundlerResult<Pubkey> {
        if let Some(pubkey) = self.public_key {
            return Ok(pubkey);
        }
        
        // In a real implementation, we would fetch the public key from KMS
        // For now, return an error indicating KMS is not fully implemented
        Err(BundlerError::Signing("KMS public key retrieval not implemented".to_string()))
    }
    
    pub async fn sign(&self, _message: &[u8]) -> BundlerResult<Signature> {
        // In a real implementation, we would use AWS KMS to sign
        // For now, return an error indicating KMS is not fully implemented
        Err(BundlerError::Signing("KMS signing not implemented".to_string()))
    }
    
    pub async fn health_check(&self) -> BundlerResult<()> {
        // In a real implementation, we would check KMS connectivity
        // For now, return an error indicating KMS is not fully implemented
        Err(BundlerError::Signing("KMS health check not implemented".to_string()))
    }
}

/// Signing manager that handles multiple key providers
pub struct SigningManager {
    fee_payer: KeyProvider,
    additional_signers: HashMap<String, KeyProvider>,
    config: SigningConfig,
}

impl SigningManager {
    /// Create a new signing manager
    pub async fn new(config: SigningConfig) -> BundlerResult<Self> {
        let fee_payer = Self::create_key_provider(&config.fee_payer).await?;
        
        let mut additional_signers = HashMap::new();
        for (alias, signer_config) in &config.additional_signers {
            let provider = Self::create_key_provider(signer_config).await?;
            additional_signers.insert(alias.clone(), provider);
        }
        
        Ok(Self {
            fee_payer,
            additional_signers,
            config,
        })
    }
    
    /// Create a key provider from configuration
    async fn create_key_provider(config: &SignerConfig) -> BundlerResult<KeyProvider> {
        match &config.signer_type {
            SignerType::File { path } => {
                let provider = FileKeyProvider::new(path)?;
                Ok(KeyProvider::File(provider))
            }
            SignerType::Env { var_name } => {
                let provider = EnvKeyProvider::new(var_name)?;
                Ok(KeyProvider::Env(provider))
            }
            SignerType::Kms { key_id, region } => {
                let region_str = region.clone().unwrap_or_else(|| "us-east-1".to_string());
                let provider = KmsKeyProvider::new(key_id.clone(), region_str);
                Ok(KeyProvider::Kms(provider))
            }
        }
    }
    
    /// Get the fee payer public key
    pub async fn fee_payer_pubkey(&self) -> BundlerResult<Pubkey> {
        self.fee_payer.public_key().await
    }
    
    /// Sign a transaction with the fee payer
    pub async fn sign_transaction(&self, transaction: &mut Transaction) -> BundlerResult<()> {
        let message = transaction.message_data();
        let signature = self.fee_payer.sign(&message).await?;
        
        // Find the fee payer signature slot and update it
        let fee_payer_pubkey = self.fee_payer.public_key().await?;
        
        for (i, pubkey) in transaction.message.account_keys.iter().enumerate() {
            if *pubkey == fee_payer_pubkey {
                if i < transaction.signatures.len() {
                    transaction.signatures[i] = signature;
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Sign a transaction with the fee payer and additional signers
    pub async fn sign_transaction_with_signers(&self, transaction: &mut Transaction, signer_aliases: &[String]) -> BundlerResult<()> {
        // First sign with fee payer
        self.sign_transaction(transaction).await?;
        
        // Then sign with additional signers
        self.sign_with_additional(transaction, signer_aliases).await?;
        
        Ok(())
    }
    
    /// Sign a transaction with additional signers
    pub async fn sign_with_additional(&self, transaction: &mut Transaction, signer_aliases: &[String]) -> BundlerResult<()> {
        for alias in signer_aliases {
            if let Some(signer) = self.additional_signers.get(alias) {
                let message = transaction.message_data();
                let signature = signer.sign(&message).await?;
                let signer_pubkey = signer.public_key().await?;
                
                // Find the signer's signature slot and update it
                for (i, pubkey) in transaction.message.account_keys.iter().enumerate() {
                    if *pubkey == signer_pubkey {
                        if i < transaction.signatures.len() {
                            transaction.signatures[i] = signature;
                            break;
                        }
                    }
                }
            } else {
                return Err(BundlerError::Signing(format!("Signer alias '{}' not found", alias)));
            }
        }
        
        Ok(())
    }
    
    /// Get a signer by alias
    pub fn get_signer(&self, alias: &str) -> Option<&KeyProvider> {
        self.additional_signers.get(alias)
    }
    
    /// Get fee payer public key
    pub async fn get_fee_payer_pubkey(&self) -> BundlerResult<Pubkey> {
        self.fee_payer.public_key().await
    }
    

    
    /// Perform health check on all signers
    pub async fn health_check(&self) -> BundlerResult<()> {
        // Check fee payer
        self.fee_payer.health_check().await
            .map_err(|e| BundlerError::Signing(format!("Fee payer health check failed: {}", e)))?;
        
        // Check additional signers
        for (alias, signer) in &self.additional_signers {
            signer.health_check().await
                .map_err(|e| BundlerError::Signing(format!("Signer '{}' health check failed: {}", alias, e)))?;
        }
        
        Ok(())
    }
    
    /// Get signing statistics
    pub async fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        stats.insert("fee_payer_configured".to_string(), serde_json::Value::Bool(true));
        stats.insert("additional_signers_count".to_string(), 
                     serde_json::Value::Number(self.additional_signers.len().into()));
        
        // Add fee payer public key if available
        if let Ok(pubkey) = self.fee_payer.public_key().await {
            stats.insert("fee_payer_pubkey".to_string(), 
                         serde_json::Value::String(pubkey.to_string()));
        }
        
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_env_key_provider() {
        // Generate a test keypair
        let keypair = Keypair::new();
        let keypair_bytes = keypair.to_bytes();
        let keypair_json = serde_json::to_string(&keypair_bytes.to_vec()).unwrap();
        
        // Set environment variable
        std::env::set_var("TEST_KEYPAIR", &keypair_json);
        
        // Create provider
        let provider = EnvKeyProvider::new("TEST_KEYPAIR").unwrap();
        
        // Test public key
        let pubkey = provider.public_key().await.unwrap();
        assert_eq!(pubkey, keypair.pubkey());
        
        // Test signing
        let message = b"test message";
        let signature = provider.sign(message).await.unwrap();
        
        // Verify signature
        assert!(signature.verify(&pubkey.to_bytes(), message));
        
        // Test health check
        assert!(provider.health_check().await.is_ok());
        
        // Clean up
        std::env::remove_var("TEST_KEYPAIR");
    }

    #[tokio::test]
    async fn test_file_key_provider() {
        // Generate a test keypair
        let keypair = Keypair::new();
        let keypair_bytes = keypair.to_bytes();
        
        // Write to temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&keypair_bytes).unwrap();
        let temp_path = temp_file.path().to_str().unwrap();
        
        // Create provider
        let provider = FileKeyProvider::new(temp_path).unwrap();
        
        // Test public key
        let pubkey = provider.public_key().await.unwrap();
        assert_eq!(pubkey, keypair.pubkey());
        
        // Test signing
        let message = b"test message";
        let signature = provider.sign(message).await.unwrap();
        
        // Verify signature
        assert!(signature.verify(&pubkey.to_bytes(), message));
        
        // Test health check
        assert!(provider.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_kms_key_provider_not_implemented() {
        let provider = KmsKeyProvider::new("test-key-id".to_string(), "us-east-1".to_string());
        
        // These should all fail since KMS is not implemented
        assert!(provider.public_key().await.is_err());
        assert!(provider.sign(b"test").await.is_err());
        assert!(provider.health_check().await.is_err());
    }
}
