
Generate key pair with

Private key:
```
openssl ecparam -genkey -name prime256v1 -out vapid_private_key.pem
```

Public key:
```
openssl ec -in vapid_private_key.pem -pubout -outform DER|tail -c 65|base64|tr '/+' '_-'|tr -d '\n=' > vapid_public_key.txt
```
