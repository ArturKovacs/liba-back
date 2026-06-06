#!/bin/bash

# The first argument to this script is the hostname
TARGET_HOST=$1

if [ -z "$TARGET_HOST" ]; then
    echo "Usage: $0 <target_host>"
    echo "Example: $0 ec2-user@10.0.0.1"
    exit 1
fi

bash remote-create-liba-service.sh "$TARGET_HOST"
bash remote-deploy-dist-folder.sh "$TARGET_HOST"

echo "Executing setup on remote host..."

# invoke user-data.sh on the target host
scp ./user-data.sh "$TARGET_HOST":/tmp/user-data.sh
ssh "$TARGET_HOST" "sudo bash /tmp/user-data.sh"

echo "Setup complete!"
