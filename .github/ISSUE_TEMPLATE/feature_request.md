name: 🚀 Feature Request
description: Suggest an idea or enhancement for StellarFraction.
title: "[FEATURE]: "
labels: ["enhancement", "discussion"]
body:
  - type: markdown
    attributes:
      value: |
        Have an idea to improve StellarFraction? Let us know! Please explain the proposed feature, its benefits, and how it aligns with our goals.
  - type: textarea
    id: problem
    attributes:
      label: Is your feature request related to a problem?
      description: A clear and concise description of what the problem is (e.g., I'm frustrated when...).
    validations:
      required: false
  - type: textarea
    id: solution
    attributes:
      label: Describe the Solution You'd Like
      description: A clear and concise description of what you want to happen. Include designs or smart contract structures if applicable.
    validations:
      required: true
  - type: textarea
    id: alternatives
    attributes:
      label: Describe Alternatives You've Considered
      description: Any alternative solutions or features you've considered.
    validations:
      required: false
  - type: textarea
    id: context
    attributes:
      label: Additional Context
      description: Add any other context or screenshots about the feature request here.
    validations:
      required: false
