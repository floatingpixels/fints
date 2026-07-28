use std::{io::Read, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Url, blocking::Client, header::CONTENT_TYPE, redirect::Policy};
use thiserror::Error;

const MAX_DECODED_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_ENCODED_RESPONSE_BYTES: usize = MAX_DECODED_RESPONSE_BYTES.div_ceil(3) * 4;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

pub(crate) struct Transport {
    endpoint: Url,
    client: Client,
}

/// Failures in the bounded HTTPS transport.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("the FinTS endpoint must be an HTTPS URL without credentials or a fragment")]
    InvalidEndpoint,
    #[error("the FinTS HTTPS client could not be constructed")]
    Client,
    #[error("the FinTS HTTPS request failed")]
    Request,
    #[error("the FinTS endpoint returned HTTP status {0}")]
    HttpStatus(u16),
    #[error("the FinTS HTTPS response exceeded the size limit")]
    ResponseTooLarge,
    #[error("the FinTS HTTPS response body was not valid Base64")]
    InvalidBase64,
}

impl Transport {
    pub(crate) fn new(endpoint: &str) -> Result<Self, TransportError> {
        let endpoint = Url::parse(endpoint).map_err(|_| TransportError::InvalidEndpoint)?;
        if endpoint.scheme() != "https"
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(TransportError::InvalidEndpoint);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .https_only(true)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| TransportError::Client)?;
        Ok(Self { endpoint, client })
    }

    pub(crate) fn send(&self, message: &[u8]) -> Result<Vec<u8>, TransportError> {
        let body = encode_body(message);
        let mut response = self
            .client
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "text/plain")
            .body(body)
            .send()
            .map_err(|_| TransportError::Request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(TransportError::HttpStatus(status.as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_ENCODED_RESPONSE_BYTES as u64)
        {
            return Err(TransportError::ResponseTooLarge);
        }

        let mut encoded = Vec::new();
        response
            .by_ref()
            .take((MAX_ENCODED_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut encoded)
            .map_err(|_| TransportError::Request)?;
        decode_body(&encoded)
    }
}

fn encode_body(message: &[u8]) -> String {
    STANDARD.encode(message)
}

fn decode_body(encoded: &[u8]) -> Result<Vec<u8>, TransportError> {
    if encoded.len() > MAX_ENCODED_RESPONSE_BYTES {
        return Err(TransportError::ResponseTooLarge);
    }
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| TransportError::InvalidBase64)?;
    if decoded.len() > MAX_DECODED_RESPONSE_BYTES {
        return Err(TransportError::ResponseTooLarge);
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ZKA HBCI 2.2 Erweiterung PIN/TAN 1.01, VI.7; FinTS 3.0 Formals,
    // Data Dictionary "Filterfunktion": the complete message uses MIME Base 64.
    #[test]
    fn complete_message_body_uses_standard_base64() {
        assert_eq!(encode_body(b"FinTS"), "RmluVFM=");
        assert_eq!(decode_body(b"RmluVFM=").unwrap(), b"FinTS");
    }

    #[test]
    fn invalid_base64_is_not_reinterpreted_as_raw_fints() {
        assert_eq!(
            decode_body(b"HNHBK:1:3+raw'").unwrap_err(),
            TransportError::InvalidBase64
        );
    }

    #[test]
    fn endpoint_must_be_https_and_must_not_embed_credentials() {
        assert_eq!(
            Transport::new("http://bank.invalid/fints").err(),
            Some(TransportError::InvalidEndpoint)
        );
        assert_eq!(
            Transport::new("https://user:secret@bank.invalid/fints").err(),
            Some(TransportError::InvalidEndpoint)
        );
    }
}
