#!/bin/bash

set -e

# Simple logging - redirect stdout and stderr to a file
exec >> /tmp/user-data.log 2>&1

echo "=== User-data script started at $(date) ==="

dnf update -y
dnf install -y nginx certbot python3-certbot-nginx

# Ensure nginx configuration directory exists
mkdir -p /etc/nginx/conf.d
echo "$(date): Nginx config directory ensured"

# Configure nginx as a reverse proxy
echo "$(date): Configuring nginx..."
cat > /etc/nginx/conf.d/liba-proxy.conf << 'EOF'
server {
    listen 80 default_server;
    server_name _;

    location / {
        proxy_pass http://localhost:3001;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }
}
EOF

# Remove default nginx configuration if it exists
rm -f /etc/nginx/conf.d/default.conf
rm -f /etc/nginx/sites-enabled/default
echo "$(date): Default nginx configs removed"

systemctl enable nginx
systemctl restart nginx
echo "$(date): Nginx restarted"

# Certbot
dnf remove certbot -y
rm -rf /opt/certbot
rm -f /usr/local/bin/certbot

python3 -m venv /opt/certbot/
/opt/certbot/bin/pip install --upgrade pip
/opt/certbot/bin/pip install certbot certbot-nginx
ln -s /opt/certbot/bin/certbot /usr/local/bin/certbot
echo "$(date): Certbot installed"

certbot --nginx -d vanbanan.hu --non-interactive --agree-tos -m kovacs.artur.barnabas@gmail.com

echo "$(date): User-data script completed successfully"
