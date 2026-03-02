import requests

url = "https://index.crates.io/ge/tr/getrandom"
response = requests.get(url)
for line in response.text.splitlines():
    import json
    data = json.loads(line)
    if data['vers'] == '0.4.1':
        print(json.dumps(data, indent=2))
        break
