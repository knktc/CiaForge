use std::io::Read;

use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::read::ZlibDecoder;

use super::{writer::{CERT_CHAIN_SIZE, TICKET_SIZE}, ConversionError};

const TICKET_AND_TMD_TEMPLATE_SIZE: usize = 0x518;

const CERT_CHAIN: &str = include_str!("../../assets/retail_certchain.zlib.b64");
const TICKET_TMD: &str = include_str!("../../assets/retail_ticket_tmd.zlib.b64");

pub struct RetailTemplates { pub certificate_chain: Vec<u8>, pub ticket_and_tmd: Vec<u8> }

impl RetailTemplates {
    pub fn load() -> Result<Self, ConversionError> {
        let certificate_chain = decode(CERT_CHAIN)?;
        if certificate_chain.len() != CERT_CHAIN_SIZE { return Err(ConversionError::Template("retail certificate chain has an unexpected length")); }
        let ticket_and_tmd = decode(TICKET_TMD)?;
        if ticket_and_tmd.len() != TICKET_AND_TMD_TEMPLATE_SIZE || ticket_and_tmd.len() < TICKET_SIZE { return Err(ConversionError::Template("retail ticket/TMD template has an unexpected length")); }
        Ok(Self { certificate_chain, ticket_and_tmd })
    }
}

fn decode(value: &str) -> Result<Vec<u8>, ConversionError> {
    let compressed = STANDARD.decode(value.trim()).map_err(|_| ConversionError::Template("invalid embedded base64"))?;
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).map_err(|_| ConversionError::Template("invalid embedded zlib data"))?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loads_expected_retail_template_sizes() {
        let templates = RetailTemplates::load().unwrap();
        assert_eq!(templates.certificate_chain.len(), CERT_CHAIN_SIZE);
        assert_eq!(templates.ticket_and_tmd.len(), TICKET_AND_TMD_TEMPLATE_SIZE);
    }
}
