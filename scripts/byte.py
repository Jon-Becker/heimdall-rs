import pyperclip

# get user input from clipboard
user_input = pyperclip.paste()

# normalize and remove ", as well as 0x
user_input = user_input.lower().replace('"', '').replace('0x', '')

# we are left with something like 363d3d373d3d3d363d73, split every 2 chars and add "0x" to the beginning. join the result with ", "
hex_array = ", ".join(["0x" + user_input[i:i+2] for i in range(0, len(user_input), 2)])

# print vec![0x36, 0x3d, 0x3d, 0x37, 0x3d, 0x3d, 0x3d, 0x36, 0x3d, 0x73]
print("&[" + hex_array + "]")

# copy to clipboard
pyperclip.copy("&[" + hex_array + "]")
