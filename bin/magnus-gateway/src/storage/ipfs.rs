//! IPFS client for storing and retrieving ISO 20022 XML messages.

use eyre::Result;
use flate2::{Compression, write::GzEncoder, read::GzDecoder};
use std::io::{Read, Write};

/// IPFS client for document storage.
#[derive(Debug, Clone)]
pub struct IpfsClient {
    api_url: String,
    gateway_url: String,
}

impl IpfsClient {
    /// Create a new IPFS client.
    pub fn new(api_url: String, gateway_url: String) -> Self {
        Self {
            api_url,
            gateway_url,
        }
    }

    /// Upload gzip-compressed data to IPFS. Returns the content hash (CID).
    pub async fn upload(&self, data: &[u8]) -> Result<String> {
        let compressed = compress(data)?;

        let client = reqwest::Client::new();
        let part = reqwest::multipart::Part::bytes(compressed)
            .file_name("message.xml.gz")
            .mime_str("application/gzip")?;
        let form = reqwest::multipart::Form::new().part("file", part);

        let resp = client
            .post(format!("{}/api/v0/add", self.api_url))
            .multipart(form)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let hash = resp
            .get("Hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing Hash in IPFS response"))?
            .to_string();

        Ok(hash)
    }

    /// Retrieve and decompress data from IPFS by hash.
    pub async fn retrieve(&self, hash: &str) -> Result<Vec<u8>> {
        let client = reqwest::Client::new();
        let url = format!("{}/ipfs/{}", self.gateway_url, hash);

        let compressed = client.get(&url).send().await?.bytes().await?;

        decompress(&compressed)
    }

    /// Pin a CID to ensure it persists.
    pub async fn pin(&self, hash: &str) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/api/v0/pin/add?arg={}", self.api_url, hash))
            .send()
            .await?;
        Ok(())
    }
}

/// Gzip-compress data.
fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

/// Gzip-decompress data.
fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"<Document>Hello ISO 20022</Document>";
        let compressed = compress(original).unwrap();
        assert!(compressed.len() < original.len() + 20); // gzip has header overhead for small data

        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_large_xml() {
        // Simulate a real XML document
        let xml = "<Document>".to_string() + &"<Entry>data</Entry>".repeat(1000) + "</Document>";
        let compressed = compress(xml.as_bytes()).unwrap();

        // Large repetitive XML should compress well
        assert!(compressed.len() < xml.len() / 5);

        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, xml.as_bytes());
    }
}
