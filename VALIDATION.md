# Release-preparation validation

Prepared 2026-09-05. These checks validate packaging and metadata, not a new
arithmetic search or physical-measurement campaign.

- Every selected verbatim file was checked against the frozen research archive's
  SHA-256 manifest. Public document/code adaptations are recorded separately in
  SOURCE_MANIFEST.json.
- The curated directory passed an automated Gitleaks scan with zero findings.
  No PDK `.lib`/`.lef` files or `.odb`/`.def` layout databases are bundled.
  Historical logs retain local directory paths for evidence provenance.
- CFF metadata passed `cffconvert 2.0.0` validation against CFF schema 1.2.0.
  Zenodo JSON parses correctly and agrees on author, title, version, and license.
  ORCID and DOI are deliberately absent until supplied/reserved by the author.
- The portable benchmark archive verified all 138 included content hashes.
  The release packager supports a downloaded source tree without Git metadata.
- The original candidate/ablation smoke check passed 4,106 deterministic vectors
  over all 75 output bits. The proof records for the six stronger controls are
  retained from the completed research campaign; they were not remeasured here.
- All local links in README.md, REPORT.md, REPRODUCING.md, NOTICE.md, and
  RELEASE_NOTES.md resolved in the curated directory.

Secret scanning is an automated check, not a guarantee that every historical
string is appropriate for every disclosure policy. The repository starts a
curated history and contains no private research-repository access settings or
account credentials. No new synthesis, routing, or optimization was run.

Public visibility, the author's ORCID, Zenodo account linkage, DOI reservation,
and actual release publication are separate from these preparation checks.
