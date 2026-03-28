//! Hand-rolled protobuf codec for the Bazel persistent worker protocol.
//!
//! The protocol is simple: length-delimited protobuf messages over stdin/stdout.
//! Only 3 message types, each with trivial fields (strings, bytes, int32).
//!
//! We avoid prost/protoc dependency to keep the build hermetic (ironic for a Bazel tool).
//!
//! Wire format reference: https://protobuf.dev/programming-guides/encoding/
//!
//! ```protobuf
//! message Input {
//!   string path = 1;
//!   bytes digest = 2;
//! }
//! message WorkRequest {
//!   repeated string arguments = 1;
//!   repeated Input inputs = 2;
//!   int32 request_id = 3;
//!   // Fields 4-7 exist in newer protocol versions but we ignore them.
//! }
//! message WorkResponse {
//!   int32 exit_code = 1;
//!   string output = 2;
//!   int32 request_id = 3;
//! }
//! ```

use std::io::{self, Read, Write};

/// A single input file provided by Bazel.
#[derive(Debug, Clone)]
pub struct Input {
    /// File path (relative to execution root).
    pub path: String,
    /// Content digest (typically SHA256, as raw bytes).
    pub digest: Vec<u8>,
}

/// A work request from Bazel.
#[derive(Debug)]
pub struct WorkRequest {
    /// Command-line arguments for this request.
    pub arguments: Vec<String>,
    /// Input files with their content digests.
    pub inputs: Vec<Input>,
    /// Request ID (non-zero for multiplex workers, 0 for singleplex).
    pub request_id: i32,
}

/// A work response sent back to Bazel.
#[derive(Debug)]
pub struct WorkResponse {
    /// 0 for success, non-zero for failure.
    pub exit_code: i32,
    /// Diagnostic output (stderr content). Bazel displays this to the user.
    pub output: String,
    /// Must match the request_id from the corresponding WorkRequest.
    pub request_id: i32,
}

// ==========================================================================
// Varint encoding/decoding
// ==========================================================================

/// Read a varint from a reader. Returns None on EOF.
fn read_varint<R: Read>(reader: &mut R) -> io::Result<Option<u64>> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    let mut buf = [0u8; 1];

    loop {
        match reader.read_exact(&mut buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                if shift == 0 {
                    return Ok(None); // Clean EOF at message boundary
                }
                return Err(e); // EOF mid-varint
            }
            Err(e) => return Err(e),
        }

        let byte = buf[0];
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(Some(result));
        }
        shift += 7;
        if shift >= 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "varint too large",
            ));
        }
    }
}

/// Write a varint to a writer.
fn write_varint<W: Write>(writer: &mut W, mut value: u64) -> io::Result<()> {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        writer.write_all(&[byte])?;
        if value == 0 {
            break;
        }
    }
    Ok(())
}

// ==========================================================================
// Low-level protobuf wire format helpers
// ==========================================================================

/// Wire types used in the protocol.
const WIRE_VARINT: u8 = 0;
const WIRE_LENGTH_DELIMITED: u8 = 2;

/// Extract field number and wire type from a tag.
fn decode_tag(tag: u64) -> (u32, u8) {
    ((tag >> 3) as u32, (tag & 0x07) as u8)
}

/// Create a tag from field number and wire type.
fn encode_tag(field: u32, wire_type: u8) -> u64 {
    ((field as u64) << 3) | (wire_type as u64)
}

/// Skip an unknown field based on its wire type.
fn skip_field(data: &[u8], pos: &mut usize, wire_type: u8) -> io::Result<()> {
    match wire_type {
        WIRE_VARINT => {
            // Skip varint bytes
            while *pos < data.len() {
                let byte = data[*pos];
                *pos += 1;
                if byte & 0x80 == 0 {
                    return Ok(());
                }
            }
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated varint"))
        }
        WIRE_LENGTH_DELIMITED => {
            let len = read_varint_from_slice(data, pos)? as usize;
            if *pos + len > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated field"));
            }
            *pos += len;
            Ok(())
        }
        0x01 => { // 64-bit
            *pos += 8;
            Ok(())
        }
        0x05 => { // 32-bit
            *pos += 4;
            Ok(())
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown wire type: {}", wire_type),
        )),
    }
}

/// Read a varint from a byte slice, advancing the position.
fn read_varint_from_slice(data: &[u8], pos: &mut usize) -> io::Result<u64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;

    loop {
        if *pos >= data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated varint"));
        }
        let byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "varint too large"));
        }
    }
}

// ==========================================================================
// WorkRequest decoding
// ==========================================================================

/// Read a length-delimited WorkRequest from a reader.
/// Returns None on clean EOF (Bazel closed stdin).
pub fn read_work_request<R: Read>(reader: &mut R) -> io::Result<Option<WorkRequest>> {
    // Read message length (varint)
    let msg_len = match read_varint(reader)? {
        Some(len) => len as usize,
        None => return Ok(None), // EOF
    };

    // Read the full message
    let mut buf = vec![0u8; msg_len];
    reader.read_exact(&mut buf)?;

    // Parse WorkRequest fields
    let mut arguments = Vec::new();
    let mut inputs = Vec::new();
    let mut request_id: i32 = 0;
    let mut pos = 0;

    while pos < buf.len() {
        let tag = read_varint_from_slice(&buf, &mut pos)?;
        let (field_num, wire_type) = decode_tag(tag);

        match (field_num, wire_type) {
            // field 1: repeated string arguments
            (1, WIRE_LENGTH_DELIMITED) => {
                let len = read_varint_from_slice(&buf, &mut pos)? as usize;
                let s = String::from_utf8(buf[pos..pos + len].to_vec())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                pos += len;
                arguments.push(s);
            }
            // field 2: repeated Input inputs
            (2, WIRE_LENGTH_DELIMITED) => {
                let len = read_varint_from_slice(&buf, &mut pos)? as usize;
                let input = decode_input(&buf[pos..pos + len])?;
                pos += len;
                inputs.push(input);
            }
            // field 3: int32 request_id
            (3, WIRE_VARINT) => {
                request_id = read_varint_from_slice(&buf, &mut pos)? as i32;
            }
            // Unknown fields: skip
            (_, wt) => {
                skip_field(&buf, &mut pos, wt)?;
            }
        }
    }

    Ok(Some(WorkRequest {
        arguments,
        inputs,
        request_id,
    }))
}

/// Decode an Input message from a byte slice.
fn decode_input(data: &[u8]) -> io::Result<Input> {
    let mut path = String::new();
    let mut digest = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let tag = read_varint_from_slice(data, &mut pos)?;
        let (field_num, wire_type) = decode_tag(tag);

        match (field_num, wire_type) {
            // field 1: string path
            (1, WIRE_LENGTH_DELIMITED) => {
                let len = read_varint_from_slice(data, &mut pos)? as usize;
                path = String::from_utf8(data[pos..pos + len].to_vec())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                pos += len;
            }
            // field 2: bytes digest
            (2, WIRE_LENGTH_DELIMITED) => {
                let len = read_varint_from_slice(data, &mut pos)? as usize;
                digest = data[pos..pos + len].to_vec();
                pos += len;
            }
            // Unknown fields: skip
            (_, wt) => {
                skip_field(data, &mut pos, wt)?;
            }
        }
    }

    Ok(Input { path, digest })
}

// ==========================================================================
// WorkResponse encoding
// ==========================================================================

/// Write a length-delimited WorkResponse to a writer.
pub fn write_work_response<W: Write>(
    writer: &mut W,
    response: &WorkResponse,
) -> io::Result<()> {
    // First, encode the message body to compute its length
    let mut body = Vec::new();

    // field 1: int32 exit_code (skip if 0)
    if response.exit_code != 0 {
        write_varint(&mut body, encode_tag(1, WIRE_VARINT))?;
        write_varint(&mut body, response.exit_code as u64)?;
    }

    // field 2: string output (skip if empty)
    if !response.output.is_empty() {
        write_varint(&mut body, encode_tag(2, WIRE_LENGTH_DELIMITED))?;
        write_varint(&mut body, response.output.len() as u64)?;
        body.write_all(response.output.as_bytes())?;
    }

    // field 3: int32 request_id (skip if 0)
    if response.request_id != 0 {
        write_varint(&mut body, encode_tag(3, WIRE_VARINT))?;
        write_varint(&mut body, response.request_id as u64)?;
    }

    // Write length prefix + body
    write_varint(writer, body.len() as u64)?;
    writer.write_all(&body)?;
    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
        for value in [0u64, 1, 127, 128, 300, 16384, u32::MAX as u64, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value).unwrap();
            let decoded = read_varint(&mut buf.as_slice()).unwrap().unwrap();
            assert_eq!(decoded, value, "varint roundtrip failed for {}", value);
        }
    }

    #[test]
    fn test_work_response_roundtrip() {
        let response = WorkResponse {
            exit_code: 0,
            output: String::new(),
            request_id: 42,
        };
        let mut buf = Vec::new();
        write_work_response(&mut buf, &response).unwrap();

        // Verify it's a valid length-delimited message
        let mut reader = buf.as_slice();
        let len = read_varint(&mut reader).unwrap().unwrap();
        assert!(len > 0);
    }

    #[test]
    fn test_work_request_decode() {
        // Build a WorkRequest manually:
        // arguments: ["--mode=schema-types", "--config=test.json"]
        // inputs: [Input { path: "schema.graphqls", digest: [0xAB, 0xCD] }]
        // request_id: 7
        let mut msg = Vec::new();

        // field 1: string "arg1"
        write_varint(&mut msg, encode_tag(1, WIRE_LENGTH_DELIMITED)).unwrap();
        let arg = b"--mode=schema-types";
        write_varint(&mut msg, arg.len() as u64).unwrap();
        msg.write_all(arg).unwrap();

        // field 1: string "arg2"
        write_varint(&mut msg, encode_tag(1, WIRE_LENGTH_DELIMITED)).unwrap();
        let arg2 = b"--config=test.json";
        write_varint(&mut msg, arg2.len() as u64).unwrap();
        msg.write_all(arg2).unwrap();

        // field 2: Input { path: "schema.graphqls", digest: [0xAB, 0xCD] }
        let mut input_msg = Vec::new();
        write_varint(&mut input_msg, encode_tag(1, WIRE_LENGTH_DELIMITED)).unwrap();
        let path = b"schema.graphqls";
        write_varint(&mut input_msg, path.len() as u64).unwrap();
        input_msg.write_all(path).unwrap();
        write_varint(&mut input_msg, encode_tag(2, WIRE_LENGTH_DELIMITED)).unwrap();
        let digest = &[0xABu8, 0xCD];
        write_varint(&mut input_msg, digest.len() as u64).unwrap();
        input_msg.write_all(digest).unwrap();

        write_varint(&mut msg, encode_tag(2, WIRE_LENGTH_DELIMITED)).unwrap();
        write_varint(&mut msg, input_msg.len() as u64).unwrap();
        msg.write_all(&input_msg).unwrap();

        // field 3: int32 request_id = 7
        write_varint(&mut msg, encode_tag(3, WIRE_VARINT)).unwrap();
        write_varint(&mut msg, 7).unwrap();

        // Wrap in length-delimited envelope
        let mut envelope = Vec::new();
        write_varint(&mut envelope, msg.len() as u64).unwrap();
        envelope.write_all(&msg).unwrap();

        // Decode
        let req = read_work_request(&mut envelope.as_slice()).unwrap().unwrap();
        assert_eq!(req.arguments.len(), 2);
        assert_eq!(req.arguments[0], "--mode=schema-types");
        assert_eq!(req.arguments[1], "--config=test.json");
        assert_eq!(req.inputs.len(), 1);
        assert_eq!(req.inputs[0].path, "schema.graphqls");
        assert_eq!(req.inputs[0].digest, vec![0xAB, 0xCD]);
        assert_eq!(req.request_id, 7);
    }

    #[test]
    fn test_eof_returns_none() {
        let empty: &[u8] = &[];
        let result = read_work_request(&mut empty.clone()).unwrap();
        assert!(result.is_none());
    }
}
