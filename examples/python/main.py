import subprocess
from typing import Optional
import tempfile
import os
import json

"""
arguments to be passed to `heimdall decompile`
"""
class DecompileArgs:
    def __init__(
        self,
        target,
        rpc_url="",
        default=False,
        skip_resolving=False,
        include_solidity=False,
        include_yul=False,
        timeout=10000,
    ):
        self.target = target
        self.rpc_url = rpc_url
        self.default = default
        self.skip_resolving = skip_resolving
        self.include_solidity = include_solidity
        self.include_yul = include_yul
        self.timeout = timeout


"""
decompiled contract
"""
class DecompiledContract:
    def __init__(self, path):
        # walk the directory and print the files
        for root, dirs, files in os.walk(path):
            for file in files:
                full_path = os.path.join(root, file)

                # read contents of the file
                # if .sol or .yul, read the contents as a string
                # if .json, read the contents as a dict
                with open(full_path, "r") as f:
                    contents = f.read()

                    if file.endswith(".sol") or file.endswith(".yul"):
                        self.source = contents
                    elif file.endswith(".json"):
                        self.abi = json.loads(contents)


"""
abstraction on top of `heimdall decompile`
"""
class Decompiler:
    def __init__(self, args: DecompileArgs):
        self.args = args

    def decompile(self) -> Optional[DecompiledContract]:
        try:
            # generate a temp dir
            temp_dir = tempfile.TemporaryDirectory()

            command = [
                "heimdall",
                "decompile",
                self.args.target,
                "--output",
                temp_dir.name,
            ]

            if self.args.rpc_url:
                command.extend(["--rpc-url", self.args.rpc_url])
            if self.args.default:
                command.append("--default")
            if self.args.skip_resolving:
                command.append("--skip-resolving")
            if self.args.include_solidity:
                command.append("--include-sol")
            if self.args.include_yul:
                command.append("--include-yul")
            if self.args.timeout:
                command.extend(["--timeout", str(self.args.timeout)])

            subprocess.check_output(command)
            return DecompiledContract(temp_dir.name)
        except Exception as e:
            print("Error: ", e)
            return None


"""
checks if the `heimdall` cli is installed on the system via `which heimdall`.
"""
def is_heimdall_installed():
    try:
        subprocess.check_output(["which", "heimdall"])
        return True
    except subprocess.CalledProcessError:
        return False


def main():
    if not is_heimdall_installed():
        print("heimdall does not seem to be installed on your system.")
        print("please install heimdall before running this script.")
        return

    args = DecompileArgs(
        target="0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        rpc_url="https://eth.llamarpc.com",
        include_solidity=True,
    )

    decompiled_contract = Decompiler(args).decompile()

    if decompiled_contract:
        print("Decompiled Contract:")
        print("Source:")
        print(decompiled_contract.source)
        print("ABI:")
        print(decompiled_contract.abi)


if __name__ == "__main__":
    main()
