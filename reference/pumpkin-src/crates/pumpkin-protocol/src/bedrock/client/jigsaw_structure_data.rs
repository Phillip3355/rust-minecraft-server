use std::io::{Error, Write};

use pumpkin_macros::packet;
use pumpkin_nbt::compound::NbtCompound;

use crate::serial::PacketWrite;

/// Transmits server-defined jigsaw structure generation rules, processors, and template pools.
#[packet(313)]
pub struct CJigsawStructureData(pub Option<&'static NbtCompound>);

impl PacketWrite for CJigsawStructureData {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        if let Some(data) = self.0 {
            return data.write(writer);
        }
        let mut data = NbtCompound::new();
        for name in ["processors", "template_pools", "jigsaws", "structure_sets"] {
            data.put_list(name, Vec::new());
        }
        data.write(writer)
    }
}
