//! Test files that the tests make at run time, so that the repository holds no binary fixtures.

/// A small PDF document with one empty page, made at run time.
pub fn pdf(title: &str) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Title ({title}) >>"),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// The start of a PNG image: its signature and its header chunk.
/// The type detection reads only the signature (ADR 0055).
pub fn png() -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(
        b"\x00\x00\x00\x0dIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00",
    );
    png.extend_from_slice(&[0; 4]);
    png
}

/// The start of an ELF executable: a program that tada must never accept.
pub fn executable() -> Vec<u8> {
    let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
    elf.resize(4096, 0);
    elf
}
