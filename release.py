import os
from pprint import pprint
import requests

token = os.environ['PERSONAL_TOKEN']

print('Getting releases')
r = requests.get('https://api.github.com/repos/saethlin/fls/releases',
                 headers={
                     "Accept": "application/vnd.github.v3+json",
                 },
                 auth=('saethlin', token))
pprint(r.json())

old_url = None
for release in r.json():
    if release['tag_name'] == 'latest':
        old_url = release['url']
        break

if old_url is not None:
    print('Deleting old release')
    r = requests.delete(old_url,
                    headers={
                        "Accept": "application/vnd.github.v3+json",
                    },
                    auth=('saethlin', token))

print('Uploading new release')
r = requests.post('https://api.github.com/repos/saethlin/fls/releases',
                  headers={
                      "Accept": "application/vnd.github.v3+json",
                  },
                  auth=('saethlin', token),
                  json={
                      'tag_name': 'latest',
                      'draft': False,
                      'prerelease': True,
                  })
pprint(r.json())

new_release = r.json()

data = open('target/release/fls', 'rb')
print('Uploading new binary...')
r = requests.post(
    'https://uploads.github.com/repos/saethlin/fls/releases/{}/assets?name=fls'
    .format(new_release['id']),
    headers={
        "Accept": "application/vnd.github.v3+json",
        "Content-Type": "application/octet-stream",
    },
    auth=('saethlin', token),
    data=data)
pprint(r.json())
print('Uploaded')
