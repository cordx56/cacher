<div align="center">
  <h1>
    🦅<br>
    cacher
  </h1>
  <p>⚡️ Faster Rust Compiler ⚡️</p>
</div>

cacher is a faster Rust compiler that utilize per-function caching of checking results.

How fast (in sec):

<table>
  <thead>
    <th>crate</th>
    <th><code>cargo build</code></th>
    <th><code>cargo cacher build</code><br>without cache</th>
    <th><code>cargo cacher build</code><br>with cache</th>
    <th>ratio</th>
  </thead>
  <tbody>
    <tr>
      <td>syn 2.0.101</td>
      <td>1.08</td>
      <td>1.20</td>
      <td>1.04</td>
      <td>96%</td>
    </tr>
    <tr>
      <td>serde 1.0.219</td>
      <td>2.03</td>
      <td>2.24</td>
      <td>1.94</td>
      <td>96%</td>
    </tr>
    <tr>
      <td>regex 1.11.1</td>
      <td>2.79</td>
      <td>2.96</td>
      <td>2.70</td>
      <td>97%</td>
    </tr>
    <tr>
      <td>tokio 1.45.1</td>
      <td>3.84</td>
      <td>4.18</td>
      <td>3.62</td>
      <td>94</td>
    </tr>
    <tr>
      <td>axum 0.8.4</td>
      <td>4.43</td>
      <td>4.93</td>
      <td>4.28</td>
      <td>97%</td>
    </tr>
  </tbody>
</table>

## Installation

Run the following commands to install both the `cacherc` compiler and the `cargo-cacher` utility:

```bash
cargo install --path . --locked
```

## Usage

Build your project with fustc by running:

```bash
cargo cacher build
```
