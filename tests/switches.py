import json
import subprocess
import difflib
import os

p = subprocess.run(['cargo', 'build', '--message-format=json'],
                   stdout=subprocess.PIPE,
                   check=True)

messages = [m for m in p.stdout.splitlines() if len(m) > 0]
exe_location = json.loads(messages[-1])['executable']

sort_orders = ['-c', '-t', '-f', '-rc', '-rt', '-rf']

for switches in sort_orders:
    fls_output = subprocess.run([exe_location, switches],
                                stdout=subprocess.PIPE,
                                check=True).stdout
    gnuls_output = subprocess.run(['/bin/ls', switches],
                                  stdout=subprocess.PIPE,
                                  check=True).stdout

    if fls_output != gnuls_output:
        print(switches, 'differs:')
        fls_output = [l.strip() for l in fls_output.decode().splitlines()]
        gnuls_output = [l.strip() for l in gnuls_output.decode().splitlines()]
        for line in difflib.Differ().compare(gnuls_output, fls_output):
            if not line.startswith(' '):
                print(line)

long_modes = ['-l', '-n', '-o', '-ln', '-lo', '-li', '-nl', '-ol', '-il']
for switches in long_modes:
    fls_output = subprocess.run([exe_location, '-f', '..', switches],
                                stdout=subprocess.PIPE,
                                check=True).stdout
    gnuls_output = subprocess.run(['/bin/ls', '-f', '..', switches],
                                  stdout=subprocess.PIPE,
                                  check=True).stdout

    if fls_output != gnuls_output:
        print(switches, 'differs:')
        for (f, g) in zip(fls_output.split(b'\n'), gnuls_output.split(b'\n')):
            if f != g:
                print('fls:', f)
                print('gnu:', g)
                print()
