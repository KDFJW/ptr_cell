# Contributing

Thank you for your interest in contributing to this project! The maintainers would be more than
happy to review and merge your pull requests

By contributing to this project, you agree that your contributions will be licensed under the dual
licensing terms of CC0 1.0 and Apache License, Version 2.0. The project maintainers reserve the
right to choose the applicable licenses for any distribution of the project. For more information on
how your contributions will be licensed, please refer to [LICENSE.md][1]

**Note**: If you are unable or unwilling to agree to these terms, you may choose to fork the project
instead

## Table of Contents
- [Getting Started](#getting-started)
- [Naming Conventions](#naming-conventions)

## Getting Started

When you're ready to contribute, follow these steps:
1. Fork the repository
2. Clone the fork to your local machine
3. Create a new branch using the naming convention outlined below
4. Make your changes, bumping the crate's [SemVer][2] version number if necessary
5. Commit the changes and push the branch to your fork
6. Open a pull request against the main repository

The GitHub [documentation][3] on creating a pull request contains additional details

## Naming Conventions

This project follows a branch naming convention for more clarity and order in the version control
history. Please use the following prefixes when creating branches:

- **`feature/`:** New features or major enhancements
  Example: `feature/more-tags`

- **`fix/`:** Bug fixes or issue resolution
  Example: `fix/memory-leak`

- **`hotfix/`:** Like **`fix/`**, but when something went *REALLY WRONG*
  Example: `hotfix/security-patch`

- **`doc/`:** Documentation updates
  Example: `doc/update-readme`

- **`test/`:** Test infrastructure updates
  Example: `test/add-unit-tests`

- **`chore/`:** Routine tasks or maintenance
  Example: `chore/update-dependencies`

- **`misc/`:** Updates that don't fall under any other category
  Example: `misc/new-branding`

As you may have noticed, words in branch names should be separated with dashes (-)

<!-- References -->
[1]: LICENSE.md
[2]: https://semver.org/
[3]: https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-a-pull-request
