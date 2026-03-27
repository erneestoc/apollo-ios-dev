// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct HeightInMeters: TestSchema.SelectionSet, Fragment {
  let __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .field("height", Height.self),
  ] }

  var height: Height { __data["height"] }

  init(
    __typename: String,
    height: Height
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": __typename,
        "height": height._fieldData,
      ],
      fulfilledFragments: [
        ObjectIdentifier(HeightInMeters.self)
      ]
    ))
  }

  /// Height
  ///
  /// Parent Type: `Height`
  struct Height: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }
    static var __selections: [Apollo.Selection] { [
      .field("__typename", String.self),
      .field("meters", Int.self),
    ] }

    var meters: Int { __data["meters"] }

    init(
      meters: Int
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Height.typename,
          "meters": meters,
        ],
        fulfilledFragments: [
          ObjectIdentifier(HeightInMeters.Height.self)
        ]
      ))
    }
  }
}
