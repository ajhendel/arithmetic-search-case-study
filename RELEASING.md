# Archive and citation maintenance

Version 1.0.0 was published on 2026-09-05 through the existing GitHub/Zenodo
integration. No manual duplicate deposit is needed.

- Exact version DOI: https://doi.org/10.5281/zenodo.22341677
- Concept DOI for all versions: https://doi.org/10.5281/zenodo.22341676
- Creator: Andrew Hendel, ORCID https://orcid.org/0009-0000-9877-3623
- Release: https://github.com/ajhendel/arithmetic-search-case-study/releases/tag/v1.0.0

The public Zenodo record was checked for creator, ORCID, version, open access,
Apache-2.0 license, and its GitHub tag relationship. The downloaded Zenodo ZIP
matched its published checksum and all 800 SOURCE_MANIFEST.json content hashes.
Zenodo archives the tagged source ZIP. The additional GitHub release assets are
convenience packages; their source and packaging tools are in the archive.

CITATION.cff cites the exact archived version. The README also links the concept
DOI, which groups all versions. ARCHIVE.json records both and the immutable
release commit. DOI links added after deposit do not change the archived tag.

Do not move v1.0.0 or replace its files merely to insert DOI links. The original
research repository remains private. No additional compute campaign is planned.

## If a substantive future version is released

Update the version and release date in CITATION.cff and .zenodo.json, update
changed content hashes in SOURCE_MANIFEST.json, and validate the release files.
Prepare a new tag and publish a new GitHub release through the enabled Zenodo
integration. Verify the new record before adding its version DOI to CITATION.cff.
Keep the concept DOI. Do not place the old version DOI in .zenodo.json; Zenodo
must assign a new version identifier. The identifier helper's --doi option is
for a separately reserved manual deposit, not this automatic release workflow.

Zenodo uses .zenodo.json for archive metadata; GitHub uses CITATION.cff for its
citation display. Keep authorship, title, license, and version consistent.
No email address or account credential is required in public citation metadata.

References: [Zenodo GitHub archiving](https://help.zenodo.org/docs/github/archive-software/github-upload/)
and [citation metadata](https://help.zenodo.org/docs/github/describe-software/citation-file/).
