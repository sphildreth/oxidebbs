pub const M_NUL: u8 = 0x00;
pub const M_ADR: u8 = 0x01;
pub const M_PWD: u8 = 0x02;
pub const M_FILE: u8 = 0x03;
pub const M_OK: u8 = 0x04;
pub const M_EOB: u8 = 0x05;
pub const M_GOT: u8 = 0x06;
pub const M_ERR: u8 = 0x07;
pub const M_BSY: u8 = 0x08;
pub const M_GET: u8 = 0x09;
pub const M_SKIP: u8 = 0x0A;

pub enum FrameType {
    Command,
    Data,
}

pub struct BinkpFrame {
    pub frame_type: FrameType,
    pub command: u8,
    pub payload: Vec<u8>,
}
