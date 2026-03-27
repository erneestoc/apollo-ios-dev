// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct PetDetailsMutation: TestSchema.MutableSelectionSet, Fragment {
  var __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .field("owner", Owner?.self),
  ] }

  var owner: Owner? {
    get { __data["owner"] }
    set { __data["owner"] = newValue }
  }

  init(
    __typename: String,
    owner: Owner? = nil
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": __typename,
        "owner": owner._fieldData,
      ],
      fulfilledFragments: [
        ObjectIdentifier(PetDetailsMutation.self)
      ]
    ))
  }

  /// Owner
  ///
  /// Parent Type: `Human`
  struct Owner: TestSchema.MutableSelectionSet {
    var __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Human }
    static var __selections: [Apollo.Selection] { [
      .field("__typename", String.self),
      .field("firstName", String.self),
    ] }

    var firstName: String {
      get { __data["firstName"] }
      set { __data["firstName"] = newValue }
    }

    init(
      firstName: String
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Human.typename,
          "firstName": firstName,
        ],
        fulfilledFragments: [
          ObjectIdentifier(PetDetailsMutation.Owner.self)
        ]
      ))
    }
  }
}
