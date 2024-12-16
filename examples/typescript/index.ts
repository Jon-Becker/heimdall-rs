import { execSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

interface DecodeArgsOptions {
  target: string;
  rpc_url?: string;
  default?: boolean;
  skip_resolving?: boolean;
  raw?: boolean;
}

class DecodeArgs {
  public target: string;
  public rpc_url: string;
  public default: boolean;
  public skip_resolving: boolean;
  public raw: boolean;

  constructor(
    target: string,
    rpc_url: string = "",
    useDefault: boolean = false,
    skip_resolving: boolean = false,
    raw: boolean = false
  ) {
    this.target = target;
    this.rpc_url = rpc_url;
    this.default = useDefault;
    this.skip_resolving = skip_resolving;
    this.raw = raw;
  }
}

class Decoder {
  private args: DecodeArgs;

  constructor(args: DecodeArgs) {
    this.args = args;
  }

  public decode(): any | null {
    try {
      const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'decoder-'));

      const command = ['decode', this.args.target, ];

      if (this.args.rpc_url) {
        command.push('--rpc-url', this.args.rpc_url);
      }
      if (this.args.default) {
        command.push('--default');
      }
      if (this.args.skip_resolving) {
        command.push('--skip-resolving');
      }
      if (this.args.raw) {
        command.push('--raw');
      }

      // Execute heimdall command
      execSync(`heimdall ${command.join(' ')}`, { stdio: 'inherit' });

      // Here you would read and parse the output from `tempDir`
      // For now, we return null since the original code doesn't show the parsing step.
      return null;
    } catch (e) {
      console.error("Error: ", e);
      return null;
    }
  }
}

function isHeimdallInstalled(): boolean {
  try {
    execSync('which heimdall', { stdio: 'pipe' });
    return true;
  } catch {
    return false;
  }
}

function main() {
  if (!isHeimdallInstalled()) {
    console.log("heimdall does not seem to be installed on your system.");
    console.log("please install heimdall before running this script.");
    return;
  }

  const args = new DecodeArgs(
    "0x000000000000000000000000008dfede2ef0e61578c3bba84a7ed4b9d25795c30000000000000000000000000000000000000001431e0fae6d7217caa00000000000000000000000000000000000000000000000000000000000000000002710fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc7c0000000000000000000000000000000000000000000000a6af776004abf4e612ad000000000000000000000000000000000000000000000000000000012a05f20000000000000000000000000000000000000000000000000000000000000111700000000000000000000000001c5f545f5b46f76e440fa02dabf88fdc0b10851a00000000000000000000000000000000000000000000000000000002540be400",
    "",
    false,
    false,
    true
  );

  const decoded = new Decoder(args).decode();
  console.log("Decoded Result:", decoded);
}

main();
