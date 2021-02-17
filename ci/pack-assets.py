# arguments: asmtpd version, target triple, target cpu

import sys
import platform
import hashlib
import shutil
import os


def sha256sum(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        data = f.read()
        h.update(data)
    return h.hexdigest()


version = sys.argv[1]
target = sys.argv[2]
target_cpu = sys.argv[3]

archive_basename = 'asmtp-{}-{}-{}'.format(version, target, target_cpu)

root_dir = './target/{}/release'.format(target)

if platform.system() == 'Windows':
    asmtpd_name = 'asmtpd.exe'
    asmtpd_cli_name = 'asmtpd-cli.exe'
    asmtp_cli_name = 'asmtp-cli.exe'
else:
    asmtpd_name = 'asmtpd'
    asmtpd_cli_name = 'asmtpd-cli'
    asmtp_cli_name = 'asmtp-cli'

asmtpd_path = os.path.join(root_dir, asmtpd_name)
asmtpd_cli_path = os.path.join(root_dir, asmtpd_cli_name)
asmtp_cli_path = os.path.join(root_dir, asmtp_cli_name)

asmtpd_checksum = sha256sum(asmtpd_path)
asmtpd_cli_checksum = sha256sum(asmtpd_cli_path)
asmtp_cli_checksum = sha256sum(asmtp_cli_path)

# build archive
if platform.system() == 'Windows':
    import zipfile
    content_type = 'application/zip'
    archive_name = '{}.zip'.format(archive_basename)
    with zipfile.ZipFile(archive_name, mode='x') as archive:
        archive.write(asmtpd_path, arcname=asmtpd_name)
        archive.write(asmtpd_cli_path, arcname=asmtpd_cli_name)
        archive.write(asmtp_cli_path, arcname=asmtp_cli_name)
else:
    import tarfile
    content_type = 'application/gzip'
    archive_name = '{}.tar.gz'.format(archive_basename)
    with tarfile.open(archive_name, 'x:gz') as archive:
        archive.add(asmtpd_path, arcname=asmtpd_name)
        archive.add(asmtpd_cli_path, arcname=asmtpd_cli_name)
        archive.add(asmtp_cli_path, arcname=asmtp_cli_name)

# verify archive
shutil.unpack_archive(archive_name, './unpack-test')
asmtpd1_checksum = sha256sum(
    os.path.join('./unpack-test', asmtpd_name))
asmtpd_cli1_checksum = sha256sum(os.path.join('./unpack-test', asmtpd_cli_name))
asmtp_cli1_checksum = sha256sum(os.path.join('./unpack-test', asmtp_cli_name))
shutil.rmtree('./unpack-test')
if asmtpd1_checksum != asmtpd_checksum:
    print('asmtpd checksum mismatch: before {} != after {}'.format(
        asmtpd_checksum, asmtpd1_checksum))
    exit(1)
if asmtpd_cli1_checksum != asmtpd_cli_checksum:
    print('asmtpd_cli checksum mismatch: before {} != after {}'.format(
        asmtpd_cli_checksum, asmtpd_cli1_checksum))
    exit(1)
if asmtp_cli1_checksum != asmtp_cli_checksum:
    print('asmtp_cli checksum mismatch: before {} != after {}'.format(
        asmtp_cli_checksum, asmtp_cli1_checksum))
    exit(1)

# save archive checksum
archive_checksum = sha256sum(archive_name)
checksum_filename = '{}.sha256'.format(archive_name)
with open(checksum_filename, 'x') as f:
    f.write(archive_checksum)

# set GitHub Action step outputs
print('::set-output name=release-archive::{}'.format(archive_name))
print('::set-output name=release-content-type::{}'.format(content_type))
