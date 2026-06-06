#!/bin/bash

# The first argument to this script is the hostname
TARGET_HOST=$1

if [ -z "$TARGET_HOST" ]; then
    echo "Usage: $0 <target_host>"
    echo "Example: $0 ec2-user@10.0.0.1"
    exit 1
fi

# Create systemd service for LIBA application
echo "$(date): Creating systemd service..."
scp ./liba-back.service "$TARGET_HOST":/tmp/liba-back.service
ssh "$TARGET_HOST" "sudo mv /tmp/liba-back.service /etc/systemd/system"

echo "$(date): Systemd service file created"

# Reload systemd daemon and start service
systemctl daemon-reload
