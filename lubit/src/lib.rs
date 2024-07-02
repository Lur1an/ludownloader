use bendy::decoding::FromBencode;
use bendy::encoding::ToBencode;
use sha1::{Digest, Sha1};
use std::str;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CompactString(compact_str::CompactString);

impl From<&str> for CompactString {
    fn from(s: &str) -> Self {
        CompactString(compact_str::CompactString::from(s))
    }
}

impl ToBencode for CompactString {
    const MAX_DEPTH: usize = 0;

    fn encode(
        &self,
        encoder: bendy::encoding::SingleItemEncoder,
    ) -> Result<(), bendy::encoding::Error> {
        encoder.emit_str(self.0.as_str())
    }
}

impl FromBencode for CompactString {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error>
    where
        Self: Sized,
    {
        let bytes = object.try_into_bytes()?;
        let str = str::from_utf8(bytes)?;
        Ok(CompactString(compact_str::CompactString::from(str)))
    }
}

impl AsRef<str> for CompactString {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub path: Vec<CompactString>,
    pub length: i64,
    pub md5sum: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub name: CompactString,
    /// Concatenated SHA-1 hashes of every piece in the torrent
    pub pieces: Vec<u8>,
    pub piece_length: i64,
    pub private: Option<bool>,
    pub info_spec: InfoSpec,
}

impl ToBencode for Info {
    const MAX_DEPTH: usize = 2;

    fn encode(
        &self,
        encoder: bendy::encoding::SingleItemEncoder,
    ) -> Result<(), bendy::encoding::Error> {
        encoder.emit_dict(|mut e| {
            match &self.info_spec {
                InfoSpec::Single(single) => {
                    e.emit_pair(b"length", single.length)?;
                    if let Some(md5sum) = &single.md5sum {
                        e.emit_pair(b"md5sum", md5sum)?;
                    }
                }
                InfoSpec::Dictionary(dict) => {
                    e.emit_pair(b"files", &dict.files)?;
                }
            }
            e.emit_pair(b"name", &self.name)?;
            e.emit_pair(b"piece length", self.piece_length)?;
            e.emit_pair(b"pieces", &self.pieces)?;
            e.emit_pair(
                b"private",
                if self.private.unwrap_or(false) { 1 } else { 0 },
            )?;
            Ok(())
        })
    }
}

impl ToBencode for File {
    const MAX_DEPTH: usize = 1;

    fn encode(
        &self,
        encoder: bendy::encoding::SingleItemEncoder,
    ) -> Result<(), bendy::encoding::Error> {
        encoder.emit_dict(|mut e| {
            e.emit_pair(b"path", &self.path)?;
            e.emit_pair(b"length", self.length)?;
            if let Some(md5sum) = &self.md5sum {
                e.emit_pair(b"md5sum", md5sum)?;
            }
            Ok(())
        })
    }
}

impl FromBencode for Info {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error> {
        let mut dict = object.try_into_dictionary()?;
        let mut piece_length = None;
        let mut pieces = None;
        let mut private = None;
        let mut name = None;
        // Single file info fields
        let mut length = None;
        let mut md5sum = None;
        // Multi file info fields
        let mut files = None;
        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"piece length", value) => {
                    piece_length = Some(i64::decode_bencode_object(value)?);
                }
                (b"pieces", value) => {
                    pieces = Some(value.try_into_bytes()?.to_vec());
                }
                (b"private", value) => {
                    private = Some(i64::decode_bencode_object(value)? == 1);
                }
                (b"name", value) => {
                    name = Some(CompactString::from(str::from_utf8(
                        value.try_into_bytes()?,
                    )?));
                }
                // Single file info fields
                (b"length", value) => {
                    length = Some(i64::decode_bencode_object(value)?);
                }
                (b"md5sum", value) => {
                    md5sum = Some(CompactString::from(str::from_utf8(
                        value.try_into_bytes()?,
                    )?));
                }
                // multi file info fields
                (b"files", value) => {
                    let mut decoded_files = vec![];
                    let mut files_decoder = value.try_into_list()?;
                    while let Some(file) = files_decoder.next_object()? {
                        let mut file_dict = file.try_into_dictionary()?;
                        let mut path = None;
                        let mut length = None;
                        let mut md5sum = None;
                        while let Some(pair) = file_dict.next_pair()? {
                            match pair {
                                (b"path", value) => {
                                    let mut path_values = vec![];
                                    let mut path_decoder = value.try_into_list()?;
                                    while let Some(path_elem) = path_decoder.next_object()? {
                                        let path_elem = CompactString::from(str::from_utf8(
                                            path_elem.try_into_bytes()?,
                                        )?);
                                        path_values.push(path_elem);
                                    }
                                    path = Some(path_values);
                                }
                                (b"length", value) => {
                                    length = Some(i64::decode_bencode_object(value)?);
                                }
                                (b"md5sum", value) => {
                                    md5sum = Some(CompactString::from(str::from_utf8(
                                        value.try_into_bytes()?,
                                    )?));
                                }
                                (unknown_key, _) => {
                                    return Err(bendy::decoding::Error::unexpected_field(
                                        String::from_utf8_lossy(unknown_key),
                                    ));
                                }
                            }
                        }
                        let path =
                            path.ok_or_else(|| bendy::decoding::Error::missing_field("path"))?;
                        let length = length
                            .ok_or_else(|| bendy::decoding::Error::missing_field("length"))?;
                        decoded_files.push(File {
                            path,
                            length,
                            md5sum,
                        });
                    }
                    files = Some(decoded_files);
                }
                (unknown_key, _) => {
                    return Err(bendy::decoding::Error::unexpected_field(
                        String::from_utf8_lossy(unknown_key),
                    ));
                }
            }
        }
        let piece_length =
            piece_length.ok_or_else(|| bendy::decoding::Error::missing_field("piece length"))?;
        let name = name.ok_or_else(|| bendy::decoding::Error::missing_field("name"))?;
        let pieces = pieces.ok_or_else(|| bendy::decoding::Error::missing_field("pieces"))?;
        let info_spec = match ((length, md5sum), files) {
            ((Some(length), md5sum), None) => InfoSpec::Single(SingleInfo { length, md5sum }),
            ((None, None), Some(files)) => InfoSpec::Dictionary(DictionaryInfo { files }),
            _ => {
                return Err(bendy::decoding::Error::unexpected_field(
                    "length/md5sum/files all present, invalid format",
                ));
            }
        };
        Ok(Info {
            name,
            pieces,
            piece_length,
            private,
            info_spec,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SingleInfo {
    pub md5sum: Option<CompactString>,
    pub length: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryInfo {
    pub files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InfoSpec {
    Single(SingleInfo),
    Dictionary(DictionaryInfo),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetaInfo {
    pub info: Info,
    pub announce: CompactString,
    pub encoding: Option<CompactString>,
    pub announce_list: Vec<Vec<CompactString>>,
    pub creation_date: Option<i64>,
    pub comment: Option<CompactString>,
    pub created_by: Option<CompactString>,
    pub url_list: Vec<CompactString>,
}

impl ToBencode for MetaInfo {
    const MAX_DEPTH: usize = Info::MAX_DEPTH + 1;

    fn encode(
        &self,
        encoder: bendy::encoding::SingleItemEncoder,
    ) -> Result<(), bendy::encoding::Error> {
        encoder.emit_dict(|mut e| {
            e.emit_pair(b"announce", &self.announce)?;
            if !self.announce_list.is_empty() {
                e.emit_pair(b"announce-list", &self.announce_list)?;
            }
            if let Some(comment) = &self.comment {
                e.emit_pair(b"comment", comment)?;
            }
            if let Some(created_by) = &self.created_by {
                e.emit_pair(b"created by", created_by)?;
            }
            if let Some(creation_date) = self.creation_date {
                e.emit_pair(b"creation date", creation_date)?;
            }
            if let Some(encoding) = &self.encoding {
                e.emit_pair(b"encoding", encoding)?;
            }
            e.emit_pair(b"info", &self.info)?;
            if !self.url_list.is_empty() {
                e.emit_pair(b"url-list", &self.url_list)?;
            }
            Ok(())
        })
    }
}

impl FromBencode for MetaInfo {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error> {
        let mut dict = object.try_into_dictionary()?;

        let mut info = None;
        let mut announce = None;
        let mut encoding = None;
        let mut announce_list = None;
        let mut creation_date = None;
        let mut comment = None;
        let mut created_by = None;
        let mut url_list = None;

        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"info", value) => {
                    info = Some(Info::decode_bencode_object(value)?);
                }
                (b"announce", value) => {
                    announce = Some(CompactString::from(str::from_utf8(
                        value.try_into_bytes()?,
                    )?));
                }
                (b"encoding", value) => {
                    encoding = Some(CompactString::from(str::from_utf8(
                        value.try_into_bytes()?,
                    )?));
                }
                (b"announce-list", value) => {
                    let mut announce_list_values = vec![];
                    let mut announce_list_decoder = value.try_into_list()?;
                    while let Some(value) = announce_list_decoder.next_object()? {
                        let mut list_decoder = value.try_into_list()?;
                        let mut inner_list = vec![];
                        while let Some(announce_elem) = list_decoder.next_object()? {
                            let announce_elem = CompactString::from(str::from_utf8(
                                announce_elem.try_into_bytes()?,
                            )?);
                            inner_list.push(announce_elem);
                        }
                        announce_list_values.push(inner_list);
                    }
                    announce_list = Some(announce_list_values);
                }
                (b"creation date", value) => {
                    creation_date = Some(i64::decode_bencode_object(value)?);
                }
                (b"comment", value) => {
                    comment = Some(CompactString::from(str::from_utf8(
                        value.try_into_bytes()?,
                    )?));
                }
                (b"created by", value) => {
                    created_by = Some(CompactString::from(str::from_utf8(
                        value.try_into_bytes()?,
                    )?));
                }
                (b"url-list", value) => {
                    let mut url_list_values = vec![];
                    let mut url_list_decoder = value.try_into_list()?;
                    while let Some(value) = url_list_decoder.next_object()? {
                        let url = str::from_utf8(value.try_into_bytes()?)?.into();
                        url_list_values.push(url);
                    }
                    url_list = Some(url_list_values);
                }
                (unknown_key, _) => {
                    return Err(bendy::decoding::Error::unexpected_field(
                        String::from_utf8_lossy(unknown_key),
                    ));
                }
            }
        }

        let info = info.ok_or_else(|| bendy::decoding::Error::missing_field("info"))?;
        let announce = announce.ok_or_else(|| bendy::decoding::Error::missing_field("announce"))?;
        let announce_list = announce_list.unwrap_or_default();
        let url_list = url_list.unwrap_or_default();

        Ok(MetaInfo {
            info,
            announce,
            encoding,
            announce_list,
            creation_date,
            comment,
            created_by,
            url_list,
        })
    }
}

fn hash_binary(data: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_fetch_torrent() {
        let torrent_file = std::fs::read("../resources/debian.torrent").unwrap();
        let meta_info = MetaInfo::from_bencode(&torrent_file).unwrap();
        println!("{:?}", meta_info);
        let encoded = meta_info.to_bencode().unwrap();
        let meta_info2 = MetaInfo::from_bencode(&encoded).unwrap();
        //println!("CHECKING EQUALITY?");
        //assert_eq!(meta_info, meta_info2);
        //assert_eq!(torrent_file, encoded);
    }
}
