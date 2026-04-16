import json
import random
import sys

def shuffle_json_array(input_path, output_path=None):
    with open(input_path, "r") as f:
        data = json.load(f)

    if not isinstance(data, list):
        raise ValueError("JSON file must contain an array at the top level")

    random.shuffle(data)

    out = output_path or input_path
    with open(out, "w") as f:
        json.dump(data, f, indent=2)

    print(f"Shuffled {len(data)} items -> {out}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python shuffle.py <input.json> [output.json]")
        sys.exit(1)
    input_path = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) > 2 else None
    shuffle_json_array(input_path, output_path)
