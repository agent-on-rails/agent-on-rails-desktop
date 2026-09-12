#!/usr/bin/env swift
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let dest = URL(fileURLWithPath: CommandLine.arguments.count > 1
  ? CommandLine.arguments[1]
  : "dmg-background.png")

let width = 660
let height = 400
let colorSpace = CGColorSpaceCreateDeviceRGB()
guard let ctx = CGContext(
  data: nil,
  width: width,
  height: height,
  bitsPerComponent: 8,
  bytesPerRow: 0,
  space: colorSpace,
  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else {
  fputs("Could not create graphics context\n", stderr)
  exit(1)
}

ctx.setFillColor(CGColor(red: 247 / 255, green: 251 / 255, blue: 1, alpha: 1))
ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))

// Arrow between app (left) and Applications (right). Finder y grows down;
// CoreGraphics y grows up, so 185px from the top is 400-185.
let cx = CGFloat(width) / 2
let cy = CGFloat(height - 185)
ctx.setStrokeColor(CGColor(red: 46 / 255, green: 182 / 255, blue: 1, alpha: 0.9))
ctx.setLineWidth(8)
ctx.setLineCap(.round)
ctx.setLineJoin(.round)

ctx.move(to: CGPoint(x: cx - 70, y: cy))
ctx.addLine(to: CGPoint(x: cx + 52, y: cy))
ctx.strokePath()

ctx.move(to: CGPoint(x: cx + 18, y: cy + 28))
ctx.addLine(to: CGPoint(x: cx + 70, y: cy))
ctx.addLine(to: CGPoint(x: cx + 18, y: cy - 28))
ctx.strokePath()

guard let image = ctx.makeImage(),
      let destination = CGImageDestinationCreateWithURL(
        dest as CFURL,
        UTType.png.identifier as CFString,
        1,
        nil
      )
else {
  fputs("Could not write PNG\n", stderr)
  exit(1)
}
CGImageDestinationAddImage(destination, image, nil)
guard CGImageDestinationFinalize(destination) else {
  fputs("Could not finalize PNG\n", stderr)
  exit(1)
}
print("Wrote \(dest.path)")
