//! A small, defensive DER reader that pulls the `uniformResourceIdentifier`
//! entries out of a certificate's `subjectAltName` extension.
//!
//! WHY hand-rolled rather than a crate: this is the ONE field SPIFFE needs from
//! the X.509 (`OID 2.5.29.17`, `GeneralName` `[6]`), and pulling in `x509-parser`
//! drags a cluster of ASN.1 crates past the dependency gates for it. The walk is
//! bounded and every length is checked against the remaining input, so a crafted
//! certificate cannot make it index out of bounds or loop — the reader returns
//! "no URI" instead of panicking. The round-trip against a real rcgen/SPIRE SAN
//! is covered by the tests here and by the live handshake (`tests/`).

/// `id-ce-subjectAltName` — the DER body of OID `2.5.29.17`.
const OID_SAN: &[u8] = &[0x55, 0x1d, 0x11];
/// Universal/constructed `SEQUENCE`.
const SEQUENCE: u8 = 0x30;
/// Universal `OBJECT IDENTIFIER`.
const OID: u8 = 0x06;
/// Universal `BOOLEAN` (an extension's optional `critical` flag).
const BOOLEAN: u8 = 0x01;
/// Universal `OCTET STRING` (an extension's `extnValue`).
const OCTET_STRING: u8 = 0x04;
/// Context-specific `[3]` constructed — the TBS `extensions` member.
const CTX_EXTENSIONS: u8 = 0xa3;
/// Context-specific `[6]` primitive — `GeneralName.uniformResourceIdentifier`.
const CTX_URI: u8 = 0x86;

/// One DER TLV split into (tag, contents, bytes-after). `None` on any malformed
/// or truncated encoding — the caller treats that as "no SAN", never a panic.
fn tlv(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&len0, rest) = rest.split_first()?;
    let (len, rest) = if len0 < 0x80 {
        (len0 as usize, rest)
    } else {
        // Long form; `0x80` (indefinite) is not valid DER and is refused here.
        let n = (len0 & 0x7f) as usize;
        if n == 0 || n > std::mem::size_of::<usize>() {
            return None;
        }
        let (lenbytes, rest) = rest.split_at_checked(n)?;
        let mut len = 0usize;
        for &b in lenbytes {
            len = (len << 8) | b as usize;
        }
        (len, rest)
    };
    let (contents, after) = rest.split_at_checked(len)?;
    Some((tag, contents, after))
}

/// Iterates the direct children of a constructed value's contents.
fn children(mut contents: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    std::iter::from_fn(move || {
        let (tag, body, rest) = tlv(contents)?;
        contents = rest;
        Some((tag, body))
    })
}

/// Every `spiffe://`-or-other URI in the certificate's `subjectAltName`, in order.
/// Empty when the cert has no SAN, no URI entry, or does not parse.
pub fn uri_sans(cert_der: &[u8]) -> Vec<String> {
    let Some(san) = san_extension(cert_der) else {
        return Vec::new();
    };
    // SubjectAltName ::= GeneralNames ::= SEQUENCE OF GeneralName.
    let Some((SEQUENCE, names, _)) = tlv(san) else {
        return Vec::new();
    };
    children(names)
        .filter(|(tag, _)| *tag == CTX_URI)
        .filter_map(|(_, body)| std::str::from_utf8(body).ok().map(str::to_owned))
        .collect()
}

/// The DER of the `subjectAltName` extension's `extnValue`, or `None`.
fn san_extension(cert_der: &[u8]) -> Option<&[u8]> {
    // Certificate ::= SEQUENCE { tbsCertificate, signatureAlgorithm, signature }.
    let (SEQUENCE, cert, _) = tlv(cert_der)? else {
        return None;
    };
    // tbsCertificate is the first child, itself a SEQUENCE.
    let (SEQUENCE, tbs, _) = tlv(cert)? else {
        return None;
    };
    // The TBS `extensions` member is the `[3]` EXPLICIT child. Scanning for the
    // tag is safe: no mandatory TBS field uses `[3]` (the others are INTEGER,
    // SEQUENCE, `[0]`, `[1]`, `[2]`).
    let exts_explicit = children(tbs).find(|(tag, _)| *tag == CTX_EXTENSIONS)?.1;
    // `[3]` EXPLICIT wraps the `Extensions` SEQUENCE OF Extension.
    let (SEQUENCE, exts, _) = tlv(exts_explicit)? else {
        return None;
    };
    for (tag, ext) in children(exts) {
        if tag != SEQUENCE {
            continue;
        }
        // Extension ::= SEQUENCE { extnID OID, critical BOOLEAN DEFAULT FALSE,
        //                          extnValue OCTET STRING }.
        let (OID, oid, after_oid) = tlv(ext)? else {
            continue;
        };
        if oid != OID_SAN {
            continue;
        }
        let (tag, body, after) = tlv(after_oid)?;
        // Skip the optional `critical` BOOLEAN if present; the next TLV is the
        // OCTET STRING whose contents are the SubjectAltName DER.
        let octet = if tag == BOOLEAN {
            let (OCTET_STRING, v, _) = tlv(after)? else {
                return None;
            };
            v
        } else if tag == OCTET_STRING {
            body
        } else {
            return None;
        };
        return Some(octet);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, Ia5String, KeyPair, SanType};

    /// A self-signed cert carrying exactly the given SANs, as DER.
    fn cert_with_sans(sans: Vec<SanType>) -> Vec<u8> {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.subject_alt_names = sans;
        let key = KeyPair::generate().unwrap();
        params.self_signed(&key).unwrap().der().to_vec()
    }

    #[test]
    fn extracts_the_uri_san() {
        let der = cert_with_sans(vec![SanType::URI(
            Ia5String::try_from("spiffe://example.org/node/web-1").unwrap(),
        )]);
        assert_eq!(uri_sans(&der), vec!["spiffe://example.org/node/web-1"]);
    }

    #[test]
    fn ignores_dns_sans() {
        let der = cert_with_sans(vec![SanType::DnsName(
            Ia5String::try_from("node.example.org").unwrap(),
        )]);
        assert!(uri_sans(&der).is_empty());
    }

    #[test]
    fn picks_the_uri_out_of_a_mixed_san() {
        let der = cert_with_sans(vec![
            SanType::DnsName(Ia5String::try_from("node.example.org").unwrap()),
            SanType::URI(Ia5String::try_from("spiffe://example.org/w").unwrap()),
        ]);
        assert_eq!(uri_sans(&der), vec!["spiffe://example.org/w"]);
    }

    #[test]
    fn garbage_in_is_empty_out_never_panics() {
        assert!(uri_sans(&[]).is_empty());
        assert!(uri_sans(&[0x30, 0x80]).is_empty()); // indefinite length
        assert!(uri_sans(&[0x30, 0x05, 0x02, 0x01, 0x01]).is_empty());
        assert!(uri_sans(&[0x30, 0xff, 0x00]).is_empty()); // length past input
    }
}
