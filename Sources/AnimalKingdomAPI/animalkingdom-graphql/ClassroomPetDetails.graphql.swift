// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct ClassroomPetDetails: TestSchema.SelectionSet, Fragment {
  let __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Unions.ClassroomPet }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .inlineFragment(AsRenamedAnimal.self),
    .inlineFragment(AsPet.self),
    .inlineFragment(AsWarmBlooded.self),
    .inlineFragment(AsCat.self),
    .inlineFragment(AsBird.self),
    .inlineFragment(AsPetRock.self),
  ] }

  var asRenamedAnimal: AsRenamedAnimal? { _asInlineFragment() }
  var asPet: AsPet? { _asInlineFragment() }
  var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }
  var asCat: AsCat? { _asInlineFragment() }
  var asBird: AsBird? { _asInlineFragment() }
  var asPetRock: AsPetRock? { _asInlineFragment() }

  init(
    __typename: String
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": __typename,
      ],
      fulfilledFragments: [
        ObjectIdentifier(ClassroomPetDetails.self)
      ]
    ))
  }

  /// AsRenamedAnimal
  ///
  /// Parent Type: `RenamedAnimal`
  struct AsRenamedAnimal: TestSchema.InlineFragment {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    typealias RootEntityType = ClassroomPetDetails
    static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
    static var __selections: [Apollo.Selection] { [
      .field("species", String.self),
    ] }

    var species: String { __data["species"] }

    init(
      __typename: String,
      species: String
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": __typename,
          "species": species,
        ],
        fulfilledFragments: [
          ObjectIdentifier(ClassroomPetDetails.self),
          ObjectIdentifier(ClassroomPetDetails.AsRenamedAnimal.self)
        ]
      ))
    }
  }

  /// AsPet
  ///
  /// Parent Type: `Pet`
  struct AsPet: TestSchema.InlineFragment {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    typealias RootEntityType = ClassroomPetDetails
    static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
    static var __selections: [Apollo.Selection] { [
      .field("humanName", String?.self),
    ] }

    var humanName: String? { __data["humanName"] }

    init(
      __typename: String,
      humanName: String? = nil
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": __typename,
          "humanName": humanName,
        ],
        fulfilledFragments: [
          ObjectIdentifier(ClassroomPetDetails.self),
          ObjectIdentifier(ClassroomPetDetails.AsPet.self)
        ]
      ))
    }
  }

  /// AsWarmBlooded
  ///
  /// Parent Type: `WarmBlooded`
  struct AsWarmBlooded: TestSchema.InlineFragment {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    typealias RootEntityType = ClassroomPetDetails
    static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
    static var __selections: [Apollo.Selection] { [
      .field("laysEggs", Bool.self),
    ] }

    var laysEggs: Bool { __data["laysEggs"] }
    var species: String { __data["species"] }

    init(
      __typename: String,
      laysEggs: Bool,
      species: String
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": __typename,
          "laysEggs": laysEggs,
          "species": species,
        ],
        fulfilledFragments: [
          ObjectIdentifier(ClassroomPetDetails.self),
          ObjectIdentifier(ClassroomPetDetails.AsWarmBlooded.self),
          ObjectIdentifier(ClassroomPetDetails.AsRenamedAnimal.self)
        ]
      ))
    }
  }

  /// AsCat
  ///
  /// Parent Type: `Cat`
  struct AsCat: TestSchema.InlineFragment {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    typealias RootEntityType = ClassroomPetDetails
    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Cat }
    static var __selections: [Apollo.Selection] { [
      .field("bodyTemperature", Int.self),
      .field("isJellicle", Bool.self),
    ] }

    var bodyTemperature: Int { __data["bodyTemperature"] }
    var isJellicle: Bool { __data["isJellicle"] }
    var species: String { __data["species"] }
    var humanName: String? { __data["humanName"] }
    var laysEggs: Bool { __data["laysEggs"] }

    init(
      bodyTemperature: Int,
      isJellicle: Bool,
      species: String,
      humanName: String? = nil,
      laysEggs: Bool
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Cat.typename,
          "bodyTemperature": bodyTemperature,
          "isJellicle": isJellicle,
          "species": species,
          "humanName": humanName,
          "laysEggs": laysEggs,
        ],
        fulfilledFragments: [
          ObjectIdentifier(ClassroomPetDetails.self),
          ObjectIdentifier(ClassroomPetDetails.AsCat.self),
          ObjectIdentifier(ClassroomPetDetails.AsRenamedAnimal.self),
          ObjectIdentifier(ClassroomPetDetails.AsPet.self),
          ObjectIdentifier(ClassroomPetDetails.AsWarmBlooded.self)
        ]
      ))
    }
  }

  /// AsBird
  ///
  /// Parent Type: `Bird`
  struct AsBird: TestSchema.InlineFragment {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    typealias RootEntityType = ClassroomPetDetails
    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Bird }
    static var __selections: [Apollo.Selection] { [
      .field("wingspan", Double.self),
    ] }

    var wingspan: Double { __data["wingspan"] }
    var species: String { __data["species"] }
    var humanName: String? { __data["humanName"] }
    var laysEggs: Bool { __data["laysEggs"] }

    init(
      wingspan: Double,
      species: String,
      humanName: String? = nil,
      laysEggs: Bool
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Bird.typename,
          "wingspan": wingspan,
          "species": species,
          "humanName": humanName,
          "laysEggs": laysEggs,
        ],
        fulfilledFragments: [
          ObjectIdentifier(ClassroomPetDetails.self),
          ObjectIdentifier(ClassroomPetDetails.AsBird.self),
          ObjectIdentifier(ClassroomPetDetails.AsRenamedAnimal.self),
          ObjectIdentifier(ClassroomPetDetails.AsPet.self),
          ObjectIdentifier(ClassroomPetDetails.AsWarmBlooded.self)
        ]
      ))
    }
  }

  /// AsPetRock
  ///
  /// Parent Type: `PetRock`
  struct AsPetRock: TestSchema.InlineFragment {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    typealias RootEntityType = ClassroomPetDetails
    static var __parentType: any Apollo.ParentType { TestSchema.Objects.PetRock }
    static var __selections: [Apollo.Selection] { [
      .field("favoriteToy", String.self),
    ] }

    var favoriteToy: String { __data["favoriteToy"] }
    var humanName: String? { __data["humanName"] }

    init(
      favoriteToy: String,
      humanName: String? = nil
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.PetRock.typename,
          "favoriteToy": favoriteToy,
          "humanName": humanName,
        ],
        fulfilledFragments: [
          ObjectIdentifier(ClassroomPetDetails.self),
          ObjectIdentifier(ClassroomPetDetails.AsPetRock.self),
          ObjectIdentifier(ClassroomPetDetails.AsPet.self)
        ]
      ))
    }
  }
}
