// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct PetDetails: TestSchema.SelectionSet, Fragment {
  let __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .field("humanName", String?.self),
    .field("favoriteToy", String.self),
    .field("owner", Owner?.self),
  ] }

  var humanName: String? { __data["humanName"] }
  var favoriteToy: String { __data["favoriteToy"] }
  var owner: Owner? { __data["owner"] }

  init(
    __typename: String,
    humanName: String? = nil,
    favoriteToy: String,
    owner: Owner? = nil
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": __typename,
        "humanName": humanName,
        "favoriteToy": favoriteToy,
        "owner": owner._fieldData,
      ],
      fulfilledFragments: [
        ObjectIdentifier(PetDetails.self)
      ]
    ))
  }

  /// Owner
  ///
  /// Parent Type: `Human`
  struct Owner: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Human }
    static var __selections: [Apollo.Selection] { [
      .field("__typename", String.self),
      .field("firstName", String.self),
    ] }

    var firstName: String { __data["firstName"] }

    init(
      firstName: String
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Human.typename,
          "firstName": firstName,
        ],
        fulfilledFragments: [
          ObjectIdentifier(PetDetails.Owner.self)
        ]
      ))
    }
  }
}
