use super::{CciHeader, cci::Partition};

const CONTENT_RECORD_SIZE: u32 = 0x30;
const BASE_TMD_SIZE: u32 = 0xb34;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentPlan {
    pub id: u32,
    pub index: u16,
    pub partition: Partition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiaPlan {
    pub title_id: u64,
    pub contents: Vec<ContentPlan>,
    pub tmd_size: u32,
    pub content_index_mask: u8,
    pub content_size: u64,
}

impl CiaPlan {
    pub fn from_header(header: &CciHeader) -> Self {
        let mut contents = vec![ContentPlan {
            id: 0,
            index: 0,
            partition: header.game,
        }];
        if let Some(partition) = header.manual {
            contents.push(ContentPlan {
                id: 1,
                index: 1,
                partition,
            });
        }
        if let Some(partition) = header.download_play_child {
            contents.push(ContentPlan {
                id: 2,
                index: 2,
                partition,
            });
        }
        let content_index_mask = contents
            .iter()
            .fold(0u8, |mask, content| mask | (0x80 >> content.index));
        let content_size = contents.iter().map(|content| content.partition.size).sum();
        let tmd_size = BASE_TMD_SIZE + CONTENT_RECORD_SIZE * (contents.len() as u32 - 1);
        Self {
            title_id: header.title_id,
            contents,
            tmd_size,
            content_index_mask,
            content_size,
        }
    }

    pub fn content_records(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.contents.len() * CONTENT_RECORD_SIZE as usize);
        for content in &self.contents {
            bytes.extend_from_slice(&content.id.to_be_bytes());
            bytes.extend_from_slice(&((content.index as u32) << 16).to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&(content.partition.size as u32).to_be_bytes());
            bytes.extend_from_slice(&[0; 32]);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{CciHeader, cci::EncryptionMode};

    fn header(manual: bool, dlp: bool) -> CciHeader {
        CciHeader {
            title_id: 0x1122334455667788,
            game: Partition {
                offset: 0x400,
                size: 0x2000,
            },
            manual: manual.then_some(Partition {
                offset: 0x2400,
                size: 0x400,
            }),
            download_play_child: dlp.then_some(Partition {
                offset: 0x2800,
                size: 0x600,
            }),
            encryption: EncryptionMode::Unencrypted,
        }
    }

    #[test]
    fn plans_all_three_content_records_in_cia_order() {
        let plan = CiaPlan::from_header(&header(true, true));
        assert_eq!(plan.contents.len(), 3);
        assert_eq!(plan.content_index_mask, 0b1110_0000);
        assert_eq!(plan.tmd_size, 0xb94);
        assert_eq!(plan.content_size, 0x2a00);
        let records = plan.content_records();
        assert_eq!(records.len(), 0x90);
        assert_eq!(&records[..4], &0u32.to_be_bytes());
        assert_eq!(&records[0x30..0x34], &1u32.to_be_bytes());
        assert_eq!(&records[0x60..0x64], &2u32.to_be_bytes());
    }

    #[test]
    fn omits_absent_optional_content() {
        let plan = CiaPlan::from_header(&header(false, false));
        assert_eq!(plan.contents.len(), 1);
        assert_eq!(plan.content_index_mask, 0b1000_0000);
        assert_eq!(plan.tmd_size, 0xb34);
    }
}
