import os
import requests

token = os.environ['PERSONAL_TOKEN']

r = requests.get('https://api.github.com/repos/saethlin/fls/releases/19681388',
                 headers={
                     "Accept": "application/vnd.github.v3+json",
                 },
                 auth=('saethlin', token))

assets = r.json()['assets']

for asset in assets:
    if asset['name'] == 'fls':
        print('Deleting existing upload...')
        requests.delete(asset['url'],
                        headers={
                            "Accept": "application/vnd.github.v3+json",
                        },
                        auth=('saethlin', token))

data = open('target/release/fls', 'rb')
print('Uploading new binary...')
r = requests.post(
    'https://uploads.github.com/repos/saethlin/fls/releases/19681388/assets?name=fls',
    headers={
        "Accept": "application/vnd.github.v3+json",
        "Content-Type": "application/octet-stream",
    },
    auth=('saethlin', token),
    data=data)

print('Uploaded')
