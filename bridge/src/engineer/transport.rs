use super::{model::*, storage::decode_limited};
use anyhow::{Result, ensure};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub async fn send<T: serde::Serialize, W: AsyncWrite + Unpin>(w: &mut W, v: &T) -> Result<()> {
    let raw = rmp_serde::to_vec_named(v)?;
    let bytes = zstd::encode_all(raw.as_slice(), 1)?;
    ensure!(bytes.len() <= 4 * 1024 * 1024, "frame limit");
    w.write_u32(bytes.len() as u32).await?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}
pub async fn receive<T: serde::de::DeserializeOwned, R: AsyncRead + Unpin>(r: &mut R) -> Result<T> {
    let len = r.read_u32().await? as usize;
    ensure!(len > 0 && len <= 4 * 1024 * 1024, "frame limit");
    let mut b = vec![0; len];
    r.read_exact(&mut b).await?;
    Ok(rmp_serde::from_slice(&decode_limited(&b)?)?)
}
pub fn certificate(root: &std::path::Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let c = root.join("receiver.der");
    let k = root.join("receiver-key.der");
    if c.exists() && k.exists() {
        return Ok((std::fs::read(c)?, std::fs::read(k)?));
    }
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["lmu-engineer".into(), "localhost".into()])?;
    let cert = cert.der().to_vec();
    let key = key_pair.serialize_der();
    std::fs::write(c, &cert)?;
    std::fs::write(k, &key)?;
    Ok((cert, key))
}
pub fn client_config(
    c: &AgentConfig,
) -> Result<std::sync::Arc<tokio_rustls::rustls::ClientConfig>> {
    use base64::Engine;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore, pki_types::CertificateDer};
    let cert = base64::engine::general_purpose::STANDARD.decode(&c.certificate_der)?;
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(cert))?;
    Ok(std::sync::Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}
