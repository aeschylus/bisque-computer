FROM node:22-slim

RUN apt-get update && apt-get install -y sudo && \
    rm -rf /var/lib/apt/lists/*

RUN npm install -g @anthropic-ai/claude-code && \
    npm cache clean --force

RUN useradd -m -s /bin/bash -G sudo claude && \
    echo 'claude ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

USER claude
WORKDIR /home/claude

# Pre-create .claude config so the first-run setup wizard is skipped.
RUN mkdir -p /home/claude/.claude && \
    echo '{"permissions":{},"hasCompletedOnboarding":true}' > /home/claude/.claude/settings.json && \
    echo '{}' > /home/claude/.claude/settings.local.json

ENTRYPOINT ["claude"]
