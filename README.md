<div align="center">
  <h1>
    🦅<br>
    fustc
  </h1>
  <p>⚡️ Faster Rust Compiler ⚡️</p>
</div>

fustc is a faster Rust compiler that utilize per-function caching of checking results.

How fast (in sec):

<table>
  <thead>
    <th>crate</th>
    <th><code>cargo build</code></th>
    <th><code>cargo fustc build</code><br>without cache</th>
    <th><code>cargo fustc build</code><br>with cache</th>
    <th>faster</th>
  </thead>
  <tbody>
    <tr>
      <td>syn 2.0.100</td>
      <td>1.070</td>
      <td>1.185</td>
      <td>1.021</td>
      <td>5%</td>
    </tr>
    <tr>
      <td>regex 1.11.1</td>
      <td>2.876</td>
      <td>3.049</td>
      <td>2.742</td>
      <td>5%</td>
    </tr>
    <tr>
      <td>tokio 1.44.1</td>
      <td>3.815</td>
      <td>4.182</td>
      <td>3.596</td>
      <td>6%</td>
    </tr>
    <tr>
      <td>axum 0.8.2</td>
      <td>4.348</td>
      <td>4.841</td>
      <td>4.090</td>
      <td>6%</td>
    </tr>
    <tr>
      <td>actix-web 4.10.2</td>
      <td>7.296</td>
      <td>8.046</td>
      <td>7.103</td>
      <td>3%</td>
    </tr>
    <tr>
      <td>tree-sitter v0.25.3</td>
      <td>8.252</td>
      <td>8.748</td>
      <td>7.723</td>
      <td>6%</td>
    </tr>
  </tbody>
</table>

## Installation

Run the following commands to install both the `fustc` compiler and the `cargo-fustc` utility:

```bash
cargo install --path fustc --locked
cargo install --path cargo-fustc --locked
```

## Usage

Build your project with fustc by running:

```bash
cargo fustc build
```
