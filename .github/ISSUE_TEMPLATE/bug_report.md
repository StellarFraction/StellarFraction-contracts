name: 🐛 Bug Report
description: File a report to help us identify and fix bugs.
title: "[BUG]: "
labels: ["bug", "triage"]
body:
  - type: markdown
    attributes:
      value: |
        Thank you for reporting a bug! Please fill out the sections below to help us reproduce and resolve the issue.
  - type: textarea
    id: description
    attributes:
      label: Bug Description
      description: Provide a clear and concise description of what the bug is.
      placeholder: E.g., The staker's claimable USDC displays as negative under certain staking amounts.
    validations:
      required: true
  - type: textarea
    id: reproduction
    attributes:
      label: Steps to Reproduce
      description: Steps to reproduce the behavior.
      placeholder: |
        1. Connect wallet...
        2. Click 'Buy & Stake'...
        3. See error...
    validations:
      required: true
  - type: textarea
    id: expected
    attributes:
      label: Expected Behavior
      description: A clear and concise description of what you expected to happen.
    validations:
      required: true
  - type: textarea
    id: environment
    attributes:
      label: Environment Info
      description: Operating system, browser version, Rust version, Stellar SDK version, etc.
      placeholder: E.g., Windows 11, Chrome 126, rustc 1.79.0, stellar-sdk 12.3.0
    validations:
      required: false
