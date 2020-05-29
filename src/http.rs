// Copyright (c) 2020 iliana destroyer of worlds <iliana@buttslol.net>
// SPDX-License-Identifier: MIT

use serde::{de::DeserializeOwned, ser::SerializeStruct, Serialize, Serializer};
use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Read, Result, Write};
use std::net::{SocketAddr, TcpStream};

fn error(err: &str) -> Error {
    Error::new(ErrorKind::InvalidData, err)
}

pub(crate) fn get<D>(addr: SocketAddr, path: &str) -> Result<(String, D)>
where
    D: DeserializeOwned,
{
    let stream = http_start(addr, "GET", path, false)?;
    let mut stream = BufReader::new(stream.into_inner()?);
    check_response_code(&mut stream)?;

    let mut buf = Vec::new();
    let mut request_id = None;
    let mut length = None;
    stream.read_until(b'\n', &mut buf)?; // finish reading status line off the wire
    loop {
        buf.clear();
        stream.read_until(b'\n', &mut buf)?;
        if buf == b"\r\n" {
            break;
        }

        if let Some((name, value)) = std::str::from_utf8(&buf).ok().and_then(split_header) {
            if request_id.is_none() && name.eq_ignore_ascii_case("Lambda-Runtime-Aws-Request-Id") {
                request_id = Some(String::from(value));
            }
            if length.is_none() {
                if name.eq_ignore_ascii_case("Transfer-Encoding") && value == "chunked" {
                    length = Some(None);
                } else if name.eq_ignore_ascii_case("Content-Length") {
                    if let Ok(value) = value.parse() {
                        length = Some(Some(value));
                    }
                }
            }
        }
    }

    let request_id = request_id.ok_or_else(|| error("missing request ID"))?;
    let event = serde_json::from_reader(
        match length.ok_or_else(|| error("can't determine body length"))? {
            Some(remaining) => Body {
                stream,
                remaining,
                chunked: false,
            },
            None => Body {
                stream,
                remaining: 0,
                chunked: true,
            },
        },
    )?;
    Ok((request_id, event))
}

pub(crate) fn post<S>(addr: SocketAddr, path: &str, body: &S) -> Result<()>
where
    S: Serialize,
{
    let mut stream = ChunkedWriter::new(http_start(addr, "POST", path, true)?);
    serde_json::to_writer(&mut stream, body)?;
    check_response_code(&mut stream.finish()?.into_inner()?)
}

pub(crate) fn post_error(addr: SocketAddr, path: &str, ty: &'static str, err: &str) -> Result<()> {
    let stream = ChunkedWriter::new(http_start(addr, "POST", path, true)?);
    let mut writer = serde_json::Serializer::new(stream);

    let mut s = writer.serialize_struct("Error", 2)?;
    s.serialize_field("errorType", ty)?;
    s.serialize_field("errorMessage", err)?;
    s.end()?;

    check_response_code(&mut writer.into_inner().finish()?.into_inner()?)
}

fn http_start(
    addr: SocketAddr,
    method: &str,
    path: &str,
    chunked: bool,
) -> Result<BufWriter<TcpStream>> {
    let mut stream = BufWriter::new(TcpStream::connect(addr)?);
    write!(
        stream,
        "{} /2018-06-01/runtime/{} HTTP/1.1\r\nhost: {}\r\n{}\r\n",
        method,
        path,
        addr,
        if chunked {
            "transfer-encoding: chunked\r\n"
        } else {
            ""
        },
    )?;
    Ok(stream)
}

fn check_response_code(mut stream: impl Read) -> Result<()> {
    let mut buf = [0; 12];
    stream.read_exact(&mut buf)?;

    if &buf[0..9] == b"HTTP/1.1 " {
        if let Some(status) = std::str::from_utf8(&buf[9..12])
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        {
            return if status >= 400 {
                Err(error(&format!("received HTTP error code {}", status)))
            } else {
                Ok(())
            };
        }
    }

    Err(error("malformed HTTP response"))
}

fn split_header(buf: &str) -> Option<(&str, &str)> {
    let mut iter = buf.splitn(2, ':');
    Some((iter.next()?, iter.next()?.trim()))
}

struct Body {
    stream: BufReader<TcpStream>,
    remaining: usize,
    chunked: bool,
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.chunked {
            if self.remaining == 0 {
                let mut len = String::new();
                self.stream.read_line(&mut len)?;
                self.remaining = usize::from_str_radix(len.trim(), 16)
                    .map_err(|_| error("invalid chunk length"))?;
                if self.remaining == 0 {
                    return Ok(0);
                }
            }

            let len = buf.len().min(self.remaining);
            let count = self.stream.read(&mut buf[..len])?;
            self.remaining -= count;
            if self.remaining == 0 {
                // read out the CRLF
                self.stream.read_exact(&mut [0; 2])?;
            }
            Ok(count)
        } else if self.remaining == 0 {
            Ok(0)
        } else {
            let len = buf.len().min(self.remaining);
            let count = self.stream.read(&mut buf[..len])?;
            self.remaining -= count;
            Ok(count)
        }
    }
}

struct ChunkedWriter(BufWriter<TcpStream>);

impl ChunkedWriter {
    pub(crate) fn new(writer: BufWriter<TcpStream>) -> ChunkedWriter {
        ChunkedWriter(writer)
    }

    pub(crate) fn finish(mut self) -> Result<BufWriter<TcpStream>> {
        self.0.write_all(b"0\r\n\r\n")?;
        Ok(self.0)
    }
}

impl Write for ChunkedWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        write!(self.0, "{:x}\r\n", buf.len())?;
        self.0.write_all(buf)?;
        self.0.write_all(b"\r\n")?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}
