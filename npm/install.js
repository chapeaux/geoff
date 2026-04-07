#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const os = require('os');

const PACKAGE_NAME = '@chapeaux/geoff';
const BINARY_NAME = 'geoff';
const REPO_OWNER = 'chapeaux';
const REPO_NAME = 'geoff';

// Determine platform and architecture
function getPlatform() {
  const platform = os.platform();
  const arch = os.arch();

  // Map Node.js platform to Rust target triple
  const platformMap = {
    'darwin': 'apple-darwin',
    'linux': 'unknown-linux-gnu',
    'win32': 'pc-windows-msvc'
  };

  const archMap = {
    'x64': 'x86_64',
    'arm64': 'aarch64'
  };

  const rustPlatform = platformMap[platform];
  const rustArch = archMap[arch];

  if (!rustPlatform || !rustArch) {
    throw new Error(`Unsupported platform: ${platform} ${arch}`);
  }

  return {
    platform,
    arch,
    target: `${rustArch}-${rustPlatform}`,
    extension: platform === 'win32' ? '.exe' : ''
  };
}

// Get the latest release version
function getLatestVersion() {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: 'api.github.com',
      path: `/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest`,
      headers: {
        'User-Agent': PACKAGE_NAME
      }
    };

    https.get(options, (res) => {
      let data = '';

      res.on('data', (chunk) => {
        data += chunk;
      });

      res.on('end', () => {
        if (res.statusCode === 200) {
          const release = JSON.parse(data);
          resolve(release.tag_name);
        } else {
          reject(new Error(`Failed to get latest release: HTTP ${res.statusCode}`));
        }
      });
    }).on('error', reject);
  });
}

// Download file with progress
function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);

    https.get(url, {
      headers: { 'User-Agent': PACKAGE_NAME }
    }, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        // Follow redirect
        return downloadFile(response.headers.location, dest)
          .then(resolve)
          .catch(reject);
      }

      if (response.statusCode !== 200) {
        reject(new Error(`Download failed: HTTP ${response.statusCode}`));
        return;
      }

      const totalSize = parseInt(response.headers['content-length'], 10);
      let downloadedSize = 0;
      let lastProgress = 0;

      response.on('data', (chunk) => {
        downloadedSize += chunk.length;
        const progress = Math.floor((downloadedSize / totalSize) * 100);

        if (progress > lastProgress && progress % 10 === 0) {
          process.stdout.write(`\rDownloading geoff: ${progress}%`);
          lastProgress = progress;
        }
      });

      response.pipe(file);

      file.on('finish', () => {
        file.close();
        console.log('\rDownload complete!        ');
        resolve();
      });
    }).on('error', (err) => {
      fs.unlink(dest, () => {});
      reject(err);
    });

    file.on('error', (err) => {
      fs.unlink(dest, () => {});
      reject(err);
    });
  });
}

// Install from pre-built binary
async function installBinary() {
  const platformInfo = getPlatform();
  const binDir = path.join(__dirname, 'bin');
  const binPath = path.join(binDir, BINARY_NAME + platformInfo.extension);

  try {
    console.log(`Installing ${BINARY_NAME} for ${platformInfo.target}...`);

    // Get latest version
    const version = await getLatestVersion();
    console.log(`Latest version: ${version}`);

    // Construct download URL
    const binaryName = `${BINARY_NAME}-${version}-${platformInfo.target}${platformInfo.extension}`;
    const downloadUrl = `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${version}/${binaryName}`;

    // Create bin directory
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    // Download binary
    await downloadFile(downloadUrl, binPath);

    // Make executable (Unix only)
    if (platformInfo.platform !== 'win32') {
      fs.chmodSync(binPath, 0o755);
    }

    console.log(`✓ ${BINARY_NAME} installed successfully!`);
    return true;
  } catch (error) {
    console.error(`Failed to install from binary: ${error.message}`);
    return false;
  }
}

// Fallback: build from source
function buildFromSource() {
  console.log('\nAttempting to build from source...');

  // Check if cargo is available
  try {
    execSync('cargo --version', { stdio: 'ignore' });
  } catch (error) {
    console.error('Error: Cargo not found. Please install Rust from https://rustup.rs/');
    process.exit(1);
  }

  try {
    console.log('Building geoff (this may take a few minutes)...');
    execSync(`cargo install chapeaux-geoff`, {
      stdio: 'inherit'
    });
    console.log('✓ Built and installed successfully!');
  } catch (error) {
    console.error('Failed to build from source:', error.message);
    console.error('\nPlease report this issue at: https://github.com/chapeaux/geoff/issues');
    process.exit(1);
  }
}

// Main installation flow
async function install() {
  console.log(`\nInstalling ${PACKAGE_NAME}...\n`);

  const success = await installBinary();

  if (!success) {
    console.log('\nBinary installation failed. Falling back to building from source...');
    buildFromSource();
  }
}

// Run installation
install().catch((error) => {
  console.error('Installation failed:', error);
  process.exit(1);
});
