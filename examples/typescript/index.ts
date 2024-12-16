import { execSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

class DecodeResult {
  public name: string;
  public signature: string;
  public inputs: any[];
  public decoded_inputs: any[];

  constructor(path: string) {
    // walk the directory and find the json file
    const files = fs.readdirSync(path);
    const jsonFile = files.find((file) => file.endsWith('.json'));
    if (!jsonFile) {
      throw new Error('`decoded.json` not found');
    }

    const data = JSON.parse(fs.readFileSync
      (path + '/' + jsonFile, 'utf8'));

    this.name = data.name;
    this.signature = data.signature;
    this.inputs = data.inputs;
    this.decoded_inputs = data.decoded_inputs;
  }
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

      const command = ['decode', this.args.target, "--output", tempDir];

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

      let result = new DecodeResult(tempDir);
      return result
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
    "0xc47f00270000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000b6a6265636b65722e657468000000000000000000000000000000000000000000",
    "",
    false,
    false,
    true
  );

  const decoded = new Decoder(args).decode();
  console.log(decoded);
}

main();
