use bendy::decoding::{Decoder, FromBencode};
use sha1::{Digest, Sha1};
use std::str;

#[derive(Debug, Clone)]
pub struct File<'a> {
    pub path: Vec<&'a str>,
    pub length: i64,
    pub md5sum: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct Info<'a> {
    pub name: &'a str,
    /// Concatenated SHA-1 hashes of every piece in the torrent
    pub pieces: &'a [u8],
    pub piece_length: i64,
    pub private: Option<bool>,
    pub info_spec: InfoSpec<'a>,
}

impl<'a> FromBencode for Info<'a> {
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
                    pieces = Some(value.try_into_bytes()?);
                }
                (b"private", value) => {
                    private = Some(i64::decode_bencode_object(value)? == 1);
                }
                (b"name", value) => {
                    name = Some(str::from_utf8(value.try_into_bytes()?)?);
                }
                // Single file info fields
                (b"length", value) => {
                    length = Some(i64::decode_bencode_object(value)?);
                }
                (b"md5sum", value) => {
                    md5sum = Some(str::from_utf8(value.try_into_bytes()?)?);
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
                                        let path_elem =
                                            str::from_utf8(path_elem.try_into_bytes()?)?;
                                        let path_elem =
                                            unsafe { std::mem::transmute::<&str, &str>(path_elem) };
                                        path_values.push(path_elem);
                                    }
                                    path = Some(path_values);
                                }
                                (b"length", value) => {
                                    length = Some(i64::decode_bencode_object(value)?);
                                }
                                (b"md5sum", value) => {
                                    md5sum = Some(str::from_utf8(value.try_into_bytes()?)?);
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
                        let md5sum = md5sum
                            .map(|md5sum| unsafe { std::mem::transmute::<_, &'a str>(md5sum) });
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
        let md5sum = md5sum.map(|md5sum| unsafe { std::mem::transmute::<&str, &'a str>(md5sum) });
        let piece_length =
            piece_length.ok_or_else(|| bendy::decoding::Error::missing_field("piece length"))?;
        let name = name
            .map(|name| unsafe { std::mem::transmute::<&str, &str>(name) })
            .ok_or_else(|| bendy::decoding::Error::missing_field("name"))?;
        let pieces = pieces
            .map(|pieces| unsafe { std::mem::transmute::<&[u8], &'a [u8]>(pieces) })
            .ok_or_else(|| bendy::decoding::Error::missing_field("pieces"))?;
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

#[derive(Debug, Clone)]
pub struct SingleInfo<'a> {
    pub md5sum: Option<&'a str>,
    pub length: i64,
}

#[derive(Debug, Clone)]
pub struct DictionaryInfo<'a> {
    pub files: Vec<File<'a>>,
}

#[derive(Debug, Clone)]
pub enum InfoSpec<'a> {
    Single(SingleInfo<'a>),
    Dictionary(DictionaryInfo<'a>),
}

#[derive(Debug, Clone)]
pub struct MetaInfo<'a> {
    pub info: Info<'a>,
    pub announce: &'a str,
    pub encoding: Option<&'a str>,
    pub announce_list: Vec<Vec<&'a str>>,
    pub creation_date: Option<i64>,
    pub comment: Option<&'a str>,
    pub created_by: Option<&'a str>,
    pub url_list: Vec<&'a str>,
}

impl<'a> FromBencode for MetaInfo<'a> {
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
                    announce = Some(str::from_utf8(value.try_into_bytes()?)?);
                }
                (b"encoding", value) => {
                    encoding = Some(str::from_utf8(value.try_into_bytes()?)?);
                }
                (b"announce-list", value) => {
                    let mut announce_list_values = vec![];
                    let mut announce_list_decoder = value.try_into_list()?;
                    while let Some(value) = announce_list_decoder.next_object()? {
                        let mut list_decoder = value.try_into_list()?;
                        let mut inner_list = vec![];
                        while let Some(announce_elem) = list_decoder.next_object()? {
                            let announce_elem = str::from_utf8(announce_elem.try_into_bytes()?)?;
                            let announce_elem =
                                unsafe { std::mem::transmute::<&str, &str>(announce_elem) };
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
                    comment = Some(str::from_utf8(value.try_into_bytes()?)?);
                }
                (b"created by", value) => {
                    created_by = Some(str::from_utf8(value.try_into_bytes()?)?);
                }
                (b"url-list", value) => {
                    let mut url_list_values = vec![];
                    let mut url_list_decoder = value.try_into_list()?;
                    while let Some(value) = url_list_decoder.next_object()? {
                        let url = str::from_utf8(value.try_into_bytes()?)?;
                        let url = unsafe { std::mem::transmute::<&str, &str>(url) };
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

        let info = info
            .map(|info| unsafe { std::mem::transmute::<Info<'_>, Info<'_>>(info) })
            .ok_or_else(|| bendy::decoding::Error::missing_field("info"))?;
        let announce = announce
            .map(|announce| unsafe { std::mem::transmute::<&str, &str>(announce) })
            .ok_or_else(|| bendy::decoding::Error::missing_field("announce"))?;
        let announce_list = announce_list.unwrap_or_default();
        let encoding = encoding.map(|e| unsafe { std::mem::transmute::<_, &'a str>(e) });
        let comment = comment.map(|c| unsafe { std::mem::transmute::<_, &'a str>(c) });
        let created_by = created_by.map(|c| unsafe { std::mem::transmute::<_, &'a str>(c) });
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

    #[tokio::test]
    async fn test_fetch_torrent() {
        let files = tokio::fs::read_dir(".").await.unwrap();
        println!("{:?}", files);
        let torrent_file = tokio::fs::read("./resources/debian.torrent").await.unwrap();
        let meta_info = MetaInfo::from_bencode(&torrent_file).unwrap();
        drop(torrent_file);
        println!("{:?}", meta_info);
    }
}
