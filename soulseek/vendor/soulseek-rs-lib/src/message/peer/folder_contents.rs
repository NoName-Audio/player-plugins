use crate::{File, FileAttributes, path::SoulseekPath, error::{Result, SoulseekRs}};
use std::io::Read;

pub fn request(token: u32, directory: &str) -> Vec<u8> {
    let mut wire = Vec::with_capacity(16 + directory.len());
    wire.extend_from_slice(&(12u32 + directory.len() as u32).to_le_bytes());
    wire.extend_from_slice(&36u32.to_le_bytes());
    wire.extend_from_slice(&token.to_le_bytes());
    wire.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    wire.extend_from_slice(directory.as_bytes()); wire
}
struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> { if n > self.0.len() { return Err(SoulseekRs::InvalidMessage("Truncated folder response".into())); } let (a,b)=self.0.split_at(n); self.0=b; Ok(a) }
    fn u32(&mut self)->Result<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn string(&mut self)->Result<String> { let n=self.u32()? as usize; String::from_utf8(self.take(n)?.to_vec()).map_err(|_|SoulseekRs::InvalidMessage("Invalid folder string".into())) }
}
pub fn response(data: &[u8], username: &str, token: u32, directory: &str) -> Result<Option<Vec<File>>> {
    let mut decoded=Vec::new();
    flate2::read::ZlibDecoder::new(data).take(16*1024*1024+1).read_to_end(&mut decoded)?;
    if decoded.len()>16*1024*1024 { return Err(SoulseekRs::InvalidMessage("Folder response too large".into())); }
    let mut c=Cursor(&decoded);
    if c.u32()? != token || c.string()? != directory { return Ok(None); }
    let mut files=Vec::new();
    let folders=c.u32()?;
    if folders>100_000 { return Err(SoulseekRs::InvalidMessage("Too many folders".into())); }
    for _ in 0..folders {
        let folder=c.string()?; let count=c.u32()?;
        if count>100_000 { return Err(SoulseekRs::InvalidMessage("Too many files".into())); }
        for _ in 0..count {
            c.take(1)?; let name=c.string()?;
            let size=u64::from_le_bytes(c.take(8)?.try_into().unwrap()); c.string()?;
            let attrs=c.u32()?; if attrs>1024 { return Err(SoulseekRs::InvalidMessage("Too many attributes".into())); }
            let mut attributes=FileAttributes::default();
            for _ in 0..attrs { let code=c.u32()?; let v=c.u32()?; match code {0=>attributes.bitrate=Some(v),1=>attributes.duration=Some(v),2=>attributes.vbr=Some(v!=0),4=>attributes.sample_rate=Some(v),5=>attributes.bit_depth=Some(v),_=>{}} }
            if folder==directory { files.push(File {username:username.into(),name:SoulseekPath::from_wire(format!("{}\\{}",folder.trim_end_matches('\\'),name)),size,attributes}); }
        }
    }
    Ok(Some(files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn outgoing_message_has_exactly_one_length_prefix() {
        let wire = request(7, "A");
        let message = crate::message::Message::new_with_data(wire[4..].to_vec());
        assert_eq!(message.get_buffer(), b"\x0d\0\0\0\x24\0\0\0\x07\0\0\0\x01\0\0\0A");
    }

    #[test]
    fn literal_request_and_compressed_response_correlate() {
        assert_eq!(request(7, "A"), b"\x0d\0\0\0\x24\0\0\0\x07\0\0\0\x01\0\0\0A");
        let plain = b"\x07\0\0\0\x01\0\0\0A\x01\0\0\0\x01\0\0\0A\x01\0\0\0\x01\x09\0\0\0cover.jpg\x7b\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(plain).unwrap(); let wire = encoder.finish().unwrap();
        let files = response(&wire, "peer", 7, "A").unwrap().unwrap();
        assert_eq!(files[0].name.as_str(), "A\\cover.jpg");
        assert_eq!(files[0].size, 123);
        assert!(response(&wire, "peer", 8, "A").unwrap().is_none());
        assert!(response(&wire, "peer", 7, "B").unwrap().is_none());
        assert!(response(&[1,2,3], "peer", 7, "A").is_err());
    }
}
