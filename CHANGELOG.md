# Changelog

この言語および処理系に対するすべての重要な変更を記録します。

## [Unreleased]


## [0.1.0] - 2026-06-13

### Changed
- Result型で帰ってくるErrの型をErrKindからErrsに変更

### Bug Fixes
- 複数の関数にfor文を書くとその次に来るfor文のループ変数が未定義になるバグを修正
