pub struct PacketHeader {
    pub orig_node: u16,
    pub orig_net: u16,
    pub orig_zone: u16,
    pub dest_node: u16,
    pub dest_net: u16,
    pub dest_zone: u16,
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub baud: u16,
    pub packet_type: u16,
    pub orig_net2: u16,
    pub dest_net2: u16,
    pub product_code: u8,
    pub password: [u8; 8],
    pub orig_zone2: u16,
    pub dest_zone2: u16,
    pub fill: [u8; 4],
}

pub struct PacketMessage {
    pub to_user: String,
    pub from_user: String,
    pub subject: String,
    pub body: Vec<u8>,
    pub area_tag: String,
}

pub struct PacketReader;

pub struct PacketWriter;
