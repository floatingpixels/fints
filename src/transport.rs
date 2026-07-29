use std::{io::Read, time::Duration};

#[cfg(test)]
use std::{cell::RefCell, collections::VecDeque};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Url, blocking::Client, header::CONTENT_TYPE, redirect::Policy};
use thiserror::Error;

const MAX_DECODED_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_ENCODED_RESPONSE_BYTES: usize = MAX_DECODED_RESPONSE_BYTES.div_ceil(3) * 4;
// RFC 2045 section 6.8 permits folding and transport whitespace. The raw
// response permits a generous but finite whitespace allowance equal to the
// encoded-data bound. Compact Base64 symbols retain their separately derived
// exact bound.
const MAX_TRANSPORT_WHITESPACE_BYTES: usize = MAX_ENCODED_RESPONSE_BYTES;
const MAX_TRANSPORT_RESPONSE_BYTES: usize =
    MAX_ENCODED_RESPONSE_BYTES + MAX_TRANSPORT_WHITESPACE_BYTES;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

pub(crate) struct Transport {
    endpoint: Url,
    client: Client,
    #[cfg(test)]
    fixture: Option<FixtureTransport>,
}

#[cfg(test)]
struct FixtureTransport {
    responses: RefCell<VecDeque<Vec<u8>>>,
    requests: RefCell<Vec<Vec<u8>>>,
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
        Ok(Self {
            endpoint,
            client,
            #[cfg(test)]
            fixture: None,
        })
    }

    pub(crate) fn send(&self, message: &[u8]) -> Result<Vec<u8>, TransportError> {
        #[cfg(test)]
        if let Some(fixture) = &self.fixture {
            fixture.requests.borrow_mut().push(message.to_vec());
            return fixture
                .responses
                .borrow_mut()
                .pop_front()
                .ok_or(TransportError::Request);
        }

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
            .is_some_and(|length| length > MAX_TRANSPORT_RESPONSE_BYTES as u64)
        {
            return Err(TransportError::ResponseTooLarge);
        }

        let mut encoded = Vec::new();
        response
            .by_ref()
            .take((MAX_TRANSPORT_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut encoded)
            .map_err(|_| TransportError::Request)?;
        decode_body(&encoded)
    }

    #[cfg(test)]
    pub(crate) fn fixture(responses: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let mut transport =
            Self::new("https://fictional.invalid/fints").expect("fictional HTTPS endpoint");
        transport.fixture = Some(FixtureTransport {
            responses: RefCell::new(responses.into_iter().collect()),
            requests: RefCell::new(Vec::new()),
        });
        transport
    }

    #[cfg(test)]
    pub(crate) fn fixture_requests(&self) -> Vec<Vec<u8>> {
        self.fixture
            .as_ref()
            .map(|fixture| fixture.requests.borrow().clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn fixture_responses_remaining(&self) -> usize {
        self.fixture
            .as_ref()
            .map(|fixture| fixture.responses.borrow().len())
            .unwrap_or_default()
    }
}

fn encode_body(message: &[u8]) -> String {
    STANDARD.encode(message)
}

fn decode_body(encoded: &[u8]) -> Result<Vec<u8>, TransportError> {
    if encoded.len() > MAX_TRANSPORT_RESPONSE_BYTES {
        return Err(TransportError::ResponseTooLarge);
    }

    // HBCI PIN/TAN VI.7 selects MIME Base 64. RFC 2045 section 6.8 requires
    // decoders to ignore line folding and whitespace, but explicitly permits
    // rejecting other non-alphabet characters as transmission errors.
    let mut compact = Vec::with_capacity(encoded.len().min(MAX_ENCODED_RESPONSE_BYTES));
    for &byte in encoded {
        if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
            continue;
        }
        if compact.len() == MAX_ENCODED_RESPONSE_BYTES {
            return Err(TransportError::ResponseTooLarge);
        }
        compact.push(byte);
    }
    if compact.is_empty() {
        return Err(TransportError::InvalidBase64);
    }

    let decoded = STANDARD
        .decode(&compact)
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

    // ZKA HBCI 2.2 PIN/TAN VI.7 selects MIME Base 64; RFC 2045 section 6.8
    // requires line folding and transport whitespace to be ignored.
    #[test]
    fn mime_folding_and_transport_whitespace_are_accepted() {
        assert_eq!(decode_body(b"Rmlu\r\nVFM=").unwrap(), b"FinTS");
        assert_eq!(decode_body(b" \t\r\nRmluVFM=\r\n\t ").unwrap(), b"FinTS");
    }

    #[test]
    fn malformed_base64_and_raw_fints_are_rejected() {
        assert_eq!(
            decode_body(b" \t\r\n").unwrap_err(),
            TransportError::InvalidBase64
        );
        assert_eq!(
            decode_body(b"Rmlu!VFM=").unwrap_err(),
            TransportError::InvalidBase64
        );
        assert_eq!(
            decode_body(b"RmluVFM===").expect_err("non-canonical padding must fail"),
            TransportError::InvalidBase64
        );
        assert_eq!(
            decode_body(b"<html>not a FinTS response</html>").unwrap_err(),
            TransportError::InvalidBase64
        );
        assert_eq!(
            decode_body(b"HNHBK:1:3+raw'").unwrap_err(),
            TransportError::InvalidBase64
        );
    }

    #[test]
    fn mime_whitespace_does_not_weaken_response_size_bounds() {
        assert_eq!(
            decode_body(&vec![b'\n'; MAX_TRANSPORT_RESPONSE_BYTES + 1]).unwrap_err(),
            TransportError::ResponseTooLarge
        );
        assert_eq!(
            decode_body(&vec![b'A'; MAX_ENCODED_RESPONSE_BYTES + 1]).unwrap_err(),
            TransportError::ResponseTooLarge
        );

        let oversized_decoded = vec![0; MAX_DECODED_RESPONSE_BYTES + 1];
        assert_eq!(
            decode_body(STANDARD.encode(oversized_decoded).as_bytes()).unwrap_err(),
            TransportError::ResponseTooLarge
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
