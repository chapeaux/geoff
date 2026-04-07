#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

const BINARY_NAME = 'geoff';

function getBinaryPath() {
  const platform = os.platform();
  const extension = platform === 'win32' ? '.exe' : '';

  // Try npm bin directory first
  const npmBinPath = path.join(__dirname, 'bin', BINARY_NAME + extension);
  if (fs.existsSync(npmBinPath)) {
    return npmBinPath;
  }

  // Try global cargo installation
  const cargoBinPath = path.join(os.homedir(), '.cargo', 'bin', BINARY_NAME + extension);
  if (fs.existsSync(cargoBinPath)) {
    return cargoBinPath;
  }

  // Fall back to PATH
  return BINARY_NAME;
}

function run() {
  const binaryPath = getBinaryPath();
  const args = process.argv.slice(2);

  const child = spawn(binaryPath, args, {
    stdio: 'inherit',
    shell: false
  });

  child.on('error', (error) => {
    if (error.code === 'ENOENT') {
      console.error(`Error: ${BINARY_NAME} binary not found.`);
      console.error('Please try reinstalling: npm install -g @chapeaux/geoff');
      console.error('\nOr install via cargo: cargo install chapeaux-geoff');
      process.exit(1);
    } else {
      console.error(`Error executing ${BINARY_NAME}:`, error.message);
      process.exit(1);
    }
  });

  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code || 0);
    }
  });
}

run();
