#!/usr/bin/env python3
"""Set a user-confirmed ORCID and/or reserved Zenodo DOI; no network actions."""
import argparse
import hashlib
import json
from pathlib import Path
import re


def orcid_id(text):
    value = text.removeprefix('https://orcid.org/').removeprefix('http://orcid.org/').strip()
    if not re.fullmatch(r'\d{4}-\d{4}-\d{4}-\d{3}[\dX]', value):
        raise argparse.ArgumentTypeError('ORCID must have the form 0000-0000-0000-0000')
    digits = value.replace('-', '')
    total = 0
    for digit in digits[:-1]:
        total = (total + int(digit)) * 2
    check = (12 - total % 11) % 11
    if digits[-1] != ('X' if check == 10 else str(check)):
        raise argparse.ArgumentTypeError('ORCID checksum does not match')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--orcid', type=orcid_id)
    parser.add_argument('--doi', help='Actual DOI reserved in your Zenodo draft; never invent one')
    args = parser.parse_args()
    if not args.orcid and not args.doi:
        parser.error('supply --orcid and/or --doi')
    doi = args.doi.removeprefix('https://doi.org/') if args.doi else None
    if doi and not re.fullmatch(r'10\.5281/zenodo\.\d+', doi):
        parser.error('expected a reserved Zenodo DOI of the form 10.5281/zenodo.NUMBER')
    root = Path(__file__).resolve().parents[1]
    citation = root / 'CITATION.cff'
    cff = citation.read_text()
    zenodo = root / '.zenodo.json'
    meta = json.loads(zenodo.read_text())
    assert meta['creators'][0]['name'] == 'Hendel, Andrew'
    if args.orcid:
        cff = re.sub(r'^    orcid:.*\n', '', cff, flags=re.M)
        cff = cff.replace('    given-names: Andrew\n',
                          f'    given-names: Andrew\n    orcid: "https://orcid.org/{args.orcid}"\n')
        meta['creators'][0]['orcid'] = args.orcid
    if doi:
        cff = re.sub(r'^doi:.*\n', '', cff, flags=re.M)
        cff += f'doi: "{doi}"\n'
        meta['doi'] = doi
    citation.write_text(cff)
    zenodo.write_text(json.dumps(meta, indent=2) + '\n')
    manifest_path = root / 'SOURCE_MANIFEST.json'
    manifest = json.loads(manifest_path.read_text())
    for p in (citation, zenodo):
        manifest['files'][p.name]['release_sha256'] = hashlib.sha256(p.read_bytes()).hexdigest()
    manifest_path.write_text(json.dumps(manifest, indent=2)+'\n')
    print('Updated citation and Zenodo metadata together; no publication performed')


if __name__ == '__main__':
    main()
