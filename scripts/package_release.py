#!/usr/bin/env python3
"""Build a self-contained archive and validate it after extraction, without a GPU."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], default='release')
    args = parser.parse_args()
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    if args.tag != f'v{version}':
        parser.error('tag must match Cargo.toml')
    windows = 'windows' in args.target
    binary = 'stargaze.exe' if windows else 'stargaze'
    name = f'stargaze-{args.tag}-{args.target}'
    dist = ROOT / 'dist'
    dist.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='stargaze-package-') as temp:
        staging = Path(temp) / name
        staging.mkdir()
        shutil.copy2(ROOT / 'target' / args.target / args.profile / binary, staging / binary)
        shutil.copytree(ROOT / 'configs', staging / 'configs')
        shutil.copy2(ROOT / 'README.md', staging / 'README.md')
        # Font license notices must accompany the fonts embedded in the binary.
        licenses = staging / 'licenses'
        licenses.mkdir()
        for filename in [ROOT / 'assets/fonts/OFL-DMSans.txt',
                         ROOT / 'prototypes/menu-ui/assets/OFL-PlexMono.txt']:
            shutil.copy2(filename, licenses / filename.name)
        shutil.copy2(ROOT / 'assets/terrain/README.md', licenses / 'Terrain-attribution.md')
        if windows:
            archive = dist / f'{name}.zip'
            with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as output:
                for path in sorted(staging.rglob('*')):
                    if path.is_file():
                        output.write(path, path.relative_to(staging.parent))
        else:
            archive = dist / f'{name}.tar.gz'
            with tarfile.open(archive, 'w:gz') as output:
                output.add(staging, arcname=name)
        # Exercise the actual archive, in a directory unrelated to the checkout.
        extracted = Path(temp) / 'extracted'
        extracted.mkdir()
        if windows:
            with zipfile.ZipFile(archive) as source:
                source.extractall(extracted)
        else:
            with tarfile.open(archive) as source:
                source.extractall(extracted, filter='data')
        installed = extracted / name
        executable = installed / binary
        result = subprocess.run([str(executable), '--version'], cwd=temp,
                                check=True, capture_output=True, text=True)
        assert result.stdout.strip() == f'stargaze {version}', result.stdout
        for config in sorted((installed / 'configs').rglob('*.yaml')):
            subprocess.run([str(executable), '--check-config', str(config)], cwd=temp, check=True)
    print(f'Created and validated {archive}')


if __name__ == '__main__':
    main()
