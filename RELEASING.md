# Author release procedure

This is a privately staged archival release. A GitHub draft is not a public
release, and a prepared metadata file is not a reserved DOI. No DOI has yet
been assigned to this artifact.

## Identity

The GitHub owner is `ajhendel`; the academic creator is **Andrew Hendel**.
Use the author's own Zenodo account, linked to the author's confirmed ORCID.
An ORCID identifies a researcher; the Zenodo DOI identifies this deposited
software-and-evidence version. Neither identifier is an email address.

The author may use a preferred account/contact email after verifying it
in Zenodo. It is not included in CITATION.cff or
.zenodo.json. Review Zenodo's profile/email visibility settings independently.
No affiliation or ORCID is inferred from a name match.

## Recommended path: one manually prepared Zenodo record

1. Sign in to the author's existing [Zenodo account](https://zenodo.org/) or
   create one using ORCID/GitHub. Confirm the account email if requested.
   Link both the author's ORCID and GitHub account in account settings.
2. Create one new draft upload of type **Software**. Copy the title, description,
   creator, version, keywords, and license from .zenodo.json. Add the actual
   ORCID to the creator. Leave the draft unpublished.
3. In the DOI field, choose that the upload does not already have a DOI and
   reserve one with “Get a DOI now!”. A reserved DOI is not yet a public record.
4. Insert the confirmed identifiers in both metadata formats:

   ```sh
   python3 scripts/set_release_identifiers.py --orcid YOUR_ORCID --doi YOUR_RESERVED_DOI
   ```

   This validates the ORCID checksum and keeps CITATION.cff, .zenodo.json,
   and their manifest hashes consistent. It performs no account or network action.
5. Review the exact repository and release files. Update the preparation-status
   text in README.md and RELEASE_NOTES.md when publication actually occurs,
   refresh the affected manifest hashes, and commit the final snapshot.
6. Rebuild the source archive and portable benchmark from that final commit.
   Upload the source archive, portable benchmark, and REPORT.md to the **same**
   Zenodo draft. Include checksums; review the creator/ORCID and all public metadata.
7. Make this curated GitHub repository public, publish the `v1.0.0` GitHub
   release with its DOI link, and publish the reviewed Zenodo record. Check both
   public links and the DOI resolution. Keep the original research repository private.
8. Optionally mark this curated repository archived/read-only once release
   metadata and links are correct. Do not archive before completing those edits.

Do not also enable automatic Zenodo/GitHub archiving for this same release:
that risks creating a second deposit for the artifact. If choosing the automatic
integration instead, follow Zenodo's GitHub workflow and use the DOI it creates;
do not independently publish a duplicate manual deposit.

## Documentation

- [Zenodo account creation](https://help.zenodo.org/docs/get-started/create-an-account/)
- [Creator names and ORCID](https://help.zenodo.org/docs/deposit/describe-records/creators/)
- [Reserve a DOI](https://help.zenodo.org/docs/deposit/describe-records/reserve-doi/)
- [Profile and email visibility](https://help.zenodo.org/docs/profile/changing-profile-visibility/)
- [CITATION.cff and .zenodo.json precedence](https://help.zenodo.org/docs/github/describe-software/citation-file/)

Zenodo uses .zenodo.json when both metadata files are supplied for its GitHub
integration; GitHub uses CITATION.cff for citation display. Keep them consistent.
No password or access token belongs in this repository or a chat message.
