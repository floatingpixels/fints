# Specification Source Register

Official Deutsche Kreditwirtschaft FinTS specifications are authoritative for this
crate. Do not implement a segment or dialog behavior until its exact source is recorded
here.

For every source, record:

- Exact document title, protocol/profile version, publication date, and revision.
- Stable official download URL or owner-controlled external path.
- File checksum when the document is stored outside the repository.
- Applicable sections and the gate/segments they support.
- Access or redistribution restrictions.

Do not commit specification PDFs unless their redistribution terms have been reviewed
and the owner explicitly requests it. Tests should cite document sections and use
independently written fictional messages rather than copied confidential or live data.

## Registered sources

None yet. Register the applicable official FinTS 3.0 and PIN/TAN documents before Gate 1
implementation begins.
