use super::CiaPlan;

pub const CIA_HEADER_FIELD: usize = 0x2020;
pub const CERT_CHAIN_SIZE: usize = 0xa00;
pub const TICKET_SIZE: usize = 0x350;
pub const META_SIZE: usize = 0x3ac0;
pub const CONTENT_RECORDS_OFFSET: usize = 0x38c4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiaHeader(pub Vec<u8>);

impl CiaHeader {
    pub fn new(
        plan: &CiaPlan,
        certificate_chain: &[u8],
        ticket_and_tmd: &[u8],
    ) -> Result<Self, &'static str> {
        if certificate_chain.len() != CERT_CHAIN_SIZE {
            return Err("certificate chain must be 0xa00 bytes");
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(CIA_HEADER_FIELD as u32).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(CERT_CHAIN_SIZE as u32).to_le_bytes());
        bytes.extend_from_slice(&(TICKET_SIZE as u32).to_le_bytes());
        bytes.extend_from_slice(&plan.tmd_size.to_le_bytes());
        bytes.extend_from_slice(&(META_SIZE as u32).to_le_bytes());
        bytes.extend_from_slice(&(plan.content_size as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.push(plan.content_index_mask);
        bytes.resize(bytes.len() + 0x201f, 0);
        bytes.extend_from_slice(certificate_chain);
        bytes.extend_from_slice(ticket_and_tmd);
        bytes.resize(bytes.len() + 0x96c, 0);
        if bytes.len() != CONTENT_RECORDS_OFFSET {
            return Err("CIA template does not align content records");
        }
        bytes.extend_from_slice(&plan.content_records());
        bytes.resize(bytes.len() + 12 + 16 * (plan.contents.len() - 1), 0);
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{
        CciHeader, CiaPlan,
        cci::{EncryptionMode, Partition},
    };

    #[test]
    fn serializes_a_fixed_size_cia_header() {
        let source = CciHeader {
            title_id: 1,
            game: Partition {
                offset: 0x400,
                size: 0x1000,
            },
            manual: None,
            download_play_child: None,
            encryption: EncryptionMode::Unencrypted,
        };
        let plan = CiaPlan::from_header(&source);
        let header = CiaHeader::new(&plan, &[0; CERT_CHAIN_SIZE], &[0; 0x518]).unwrap();
        assert!(header.0.len() > 0x2040);
        assert_eq!(&header.0[..4], &(CIA_HEADER_FIELD as u32).to_le_bytes());
        assert_eq!(header.0[0x20], 0x80);
        assert_eq!(header.0.len(), 0x3900);
    }
}
