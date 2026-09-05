# Author release procedure

This is a privately staged archival release. No DOI has yet been assigned to
this particular artifact. The author already has an established Zenodo account
and active GitHub/Zenodo integrations for earlier projects.

## Verified identity and existing records

- GitHub owner and creator: `ajhendel`, **Andrew Hendel**.
- ORCID: [0009-0000-9877-3623](https://orcid.org/0009-0000-9877-3623).
- [Public mojolearn citation](https://github.com/mojolearn/mojolearn/blob/main/CITATION.cff)
  identifies Andrew Hendel, alias ajhendel, with that ORCID.
- [Existing mojolearn archive](https://doi.org/10.5281/zenodo.22068632) includes
  the matching creator/ORCID in Zenodo's record metadata.
- [Existing tinytapeout1 archive](https://doi.org/10.5281/zenodo.22261255) names
  Andrew Hendel. Both records have the same Zenodo owner ID, 1730602.

The existing DOIs identify different artifacts. Do not reuse either DOI for this
case study. The user's verified account email is managed on Zenodo; an email
address is not required in public citation metadata and is not included here.

## Use the existing GitHub/Zenodo integration

The original tinytapeout1 and mojolearn repositories already have active
`zenodo.org` webhooks. The newly created private case-study repository does not.
No new Zenodo account is needed.

1. Review the curated GitHub draft release and final metadata. When ready for
   public publication, make this curated repository public; keep the original
   tinytapeout2 research repository private.
2. In the author's existing Zenodo account, open the GitHub integration settings,
   sync repositories, and enable `ajhendel/arithmetic-search-case-study`.
   Do not create a second account for the same author or a duplicate manual deposit.
3. Update README.md and RELEASE_NOTES.md preparation wording when publishing,
   refresh affected manifest hashes, commit, rebuild/check the assets, and update
   the GitHub draft release target and assets to that exact commit.
4. Publish the reviewed `v1.0.0` GitHub release after the Zenodo integration is
   enabled. Zenodo should archive it and issue this artifact's own DOI.
5. Verify the Zenodo record's creator, ORCID, version, license, and uploaded
   content. Link its DOI from GitHub and update citation metadata for subsequent
   repository use. Use the concept DOI for the evolving software citation and
   the version DOI for an exact archived release. Do not retag an archived version
   merely to insert a DOI into its already-deposited source files.
6. Once metadata and links are verified, optionally archive the GitHub repository
   as read-only. No further experiments are required for this step.

If a DOI is needed inside the initial source files before publication, use a
single manual Zenodo draft and reserve its DOI instead. In that case do not also
activate automatic ingestion for the same release. The identifier helper updates
both metadata formats and validates the ORCID checksum:

```sh
python3 scripts/set_release_identifiers.py --orcid 0009-0000-9877-3623 --doi YOUR_RESERVED_DOI
```

That helper performs no account or network action.

## Documentation

- [Enable a repository in Zenodo](https://help.zenodo.org/docs/github/enable-repository/)
- [Archive a GitHub release](https://help.zenodo.org/docs/github/archive-software/github-upload/)
- [Reserve a DOI for a manual draft](https://help.zenodo.org/docs/deposit/describe-records/reserve-doi/)
- [CITATION.cff and .zenodo.json precedence](https://help.zenodo.org/docs/github/describe-software/citation-file/)

Zenodo uses .zenodo.json when both metadata files are supplied for its GitHub
integration; GitHub uses CITATION.cff for citation display. Keep them consistent.
No password or access token belongs in this repository or a chat message.
