// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct WarmBloodedDetails: TestSchema.SelectionSet, Fragment {
  let __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .field("bodyTemperature", Int.self),
    .fragment(HeightInMeters.self),
  ] }

  var bodyTemperature: Int { __data["bodyTemperature"] }
  var height: Height { __data["height"] }

  struct Fragments: FragmentContainer {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    var heightInMeters: HeightInMeters { _toFragment() }
  }

  init(
    __typename: String,
    bodyTemperature: Int,
    height: Height
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": __typename,
        "bodyTemperature": bodyTemperature,
        "height": height._fieldData,
      ],
      fulfilledFragments: [
        ObjectIdentifier(WarmBloodedDetails.self),
        ObjectIdentifier(HeightInMeters.self)
      ]
    ))
  }

  typealias Height = HeightInMeters.Height
}
