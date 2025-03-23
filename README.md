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
      <td>1.150</td>
      <td>1.228</td>
      <td>1.071</td>
      <td>7%</td>
    </tr>
    <tr>
      <td>regex 1.11.1</td>
      <td>2.937</td>
      <td>3.058</td>
      <td>2.789</td>
      <td>5%</td>
    </tr>
    <tr>
      <td>tokio 1.44.1</td>
      <td>3.998</td>
      <td>4.256</td>
      <td>3.662</td>
      <td>8%</td>
    </tr>
    <tr>
      <td>axum 0.8.2</td>
      <td>4.534</td>
      <td>4.908</td>
      <td>4.140</td>
      <td>9%</td>
    </tr>
    <tr>
      <td>actix-web 4.10.2</td>
      <td>7.546</td>
      <td>8.026</td>
      <td>7.065</td>
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
