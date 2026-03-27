// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class AllAnimalsIncludeSkipQuery: GraphQLQuery {
  static let operationName: String = "AllAnimalsIncludeSkipQuery"
  static let operationDocument: Apollo.OperationDocument = .init(
    operationIdentifier: "e0b15244cded59b826b21adf8cb16cbcee1c303a06434bc6148d955c81cd7fda"
  )

  var includeSpecies: Bool
  var skipHeightInMeters: Bool
  var getCat: Bool
  var getWarmBlooded: Bool
  var varA: Bool

  init(
    includeSpecies: Bool,
    skipHeightInMeters: Bool,
    getCat: Bool,
    getWarmBlooded: Bool,
    varA: Bool
  ) {
    self.includeSpecies = includeSpecies
    self.skipHeightInMeters = skipHeightInMeters
    self.getCat = getCat
    self.getWarmBlooded = getWarmBlooded
    self.varA = varA
  }

  var __variables: Variables? { [
    "includeSpecies": includeSpecies,
    "skipHeightInMeters": skipHeightInMeters,
    "getCat": getCat,
    "getWarmBlooded": getWarmBlooded,
    "varA": varA
  ] }

  struct Data: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("allAnimals", [AllAnimal].self),
    ] }

    var allAnimals: [AllAnimal] { __data["allAnimals"] }

    /// AllAnimal
    ///
    /// Parent Type: `RenamedAnimal`
    struct AllAnimal: TestSchema.SelectionSet {
      let __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("height", Height.self),
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .field("predators", [Predator].self),
        .inlineFragment(AsPet.self),
        .inlineFragment(AsClassroomPet.self),
        .include(if: "includeSpecies", .field("species", String.self)),
        .include(if: !"skipHeightInMeters", .inlineFragment(IfNotSkipHeightInMeters.self)),
        .include(if: "getWarmBlooded", .inlineFragment(AsWarmBloodedIfGetWarmBlooded.self)),
        .include(if: "getCat", .inlineFragment(AsCatIfGetCat.self)),
      ] }

      var height: Height { __data["height"] }
      var species: String? { __data["species"] }
      var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
      var predators: [Predator] { __data["predators"] }

      var ifNotSkipHeightInMeters: IfNotSkipHeightInMeters? { _asInlineFragment() }
      var asWarmBloodedIfGetWarmBlooded: AsWarmBloodedIfGetWarmBlooded? { _asInlineFragment() }
      var asPet: AsPet? { _asInlineFragment() }
      var asCatIfGetCat: AsCatIfGetCat? { _asInlineFragment() }
      var asClassroomPet: AsClassroomPet? { _asInlineFragment() }

      struct Fragments: FragmentContainer {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        var heightInMeters: HeightInMeters? { _toFragment() }
      }

      /// AllAnimal.Height
      ///
      /// Parent Type: `Height`
      struct Height: TestSchema.SelectionSet {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }
        static var __selections: [Apollo.Selection] { [
          .field("__typename", String.self),
          .field("feet", Int.self),
          .field("inches", Int?.self),
        ] }

        var feet: Int { __data["feet"] }
        var inches: Int? { __data["inches"] }
      }

      /// AllAnimal.Predator
      ///
      /// Parent Type: `RenamedAnimal`
      struct Predator: TestSchema.SelectionSet {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
        static var __selections: [Apollo.Selection] { [
          .field("__typename", String.self),
          .include(if: "includeSpecies", .field("species", String.self)),
          .include(if: "getWarmBlooded", .inlineFragment(AsWarmBloodedIfGetWarmBlooded.self)),
        ] }

        var species: String? { __data["species"] }

        var asWarmBloodedIfGetWarmBlooded: AsWarmBloodedIfGetWarmBlooded? { _asInlineFragment() }

        /// AllAnimal.Predator.AsWarmBloodedIfGetWarmBlooded
        ///
        /// Parent Type: `WarmBlooded`
        struct AsWarmBloodedIfGetWarmBlooded: TestSchema.InlineFragment {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal.Predator
          static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
          static var __selections: [Apollo.Selection] { [
            .field("species", String.self),
            .fragment(WarmBloodedDetails.self),
            .field("laysEggs", Bool.self),
          ] }

          var species: String { __data["species"] }
          var laysEggs: Bool { __data["laysEggs"] }
          var bodyTemperature: Int { __data["bodyTemperature"] }
          var height: Height { __data["height"] }

          struct Fragments: FragmentContainer {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            var heightInMeters: HeightInMeters { _toFragment() }
          }

          typealias Height = HeightInMeters.Height
        }
      }

      /// AllAnimal.IfNotSkipHeightInMeters
      ///
      /// Parent Type: `RenamedAnimal`
      struct IfNotSkipHeightInMeters: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
        static var __selections: [Apollo.Selection] { [
          .fragment(HeightInMeters.self),
        ] }

        var height: Height { __data["height"] }
        var species: String? { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.IfNotSkipHeightInMeters.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
        }
      }

      /// AllAnimal.AsWarmBloodedIfGetWarmBlooded
      ///
      /// Parent Type: `WarmBlooded`
      struct AsWarmBloodedIfGetWarmBlooded: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
        static var __selections: [Apollo.Selection] { [
          .fragment(WarmBloodedDetails.self),
        ] }

        var height: Height { __data["height"] }
        var species: String? { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var bodyTemperature: Int { __data["bodyTemperature"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsWarmBloodedIfGetWarmBlooded.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
        }
      }

      /// AllAnimal.AsPet
      ///
      /// Parent Type: `Pet`
      struct AsPet: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
        static var __selections: [Apollo.Selection] { [
          .field("height", Height.self),
          .inlineFragment(AsWarmBlooded.self),
          .fragment(PetDetails.self),
        ] }

        var height: Height { __data["height"] }
        var species: String? { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var humanName: String? { __data["humanName"] }
        var favoriteToy: String { __data["favoriteToy"] }
        var owner: Owner? { __data["owner"] }

        var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var petDetails: PetDetails { _toFragment() }
          var heightInMeters: HeightInMeters? { _toFragment() }
        }

        /// AllAnimal.AsPet.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }
          static var __selections: [Apollo.Selection] { [
            .field("__typename", String.self),
            .include(if: "varA", [
              .field("relativeSize", GraphQLEnum<TestSchema.RelativeSize>.self),
              .field("centimeters", Double.self),
            ]),
          ] }

          var relativeSize: GraphQLEnum<TestSchema.RelativeSize>? { __data["relativeSize"] }
          var centimeters: Double? { __data["centimeters"] }
          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
        }

        typealias Owner = PetDetails.Owner

        /// AllAnimal.AsPet.AsWarmBlooded
        ///
        /// Parent Type: `WarmBlooded`
        struct AsWarmBlooded: TestSchema.InlineFragment {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
          static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
          static var __selections: [Apollo.Selection] { [
            .fragment(WarmBloodedDetails.self),
          ] }

          var height: Height { __data["height"] }
          var species: String? { __data["species"] }
          var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
          var predators: [Predator] { __data["predators"] }
          var bodyTemperature: Int { __data["bodyTemperature"] }
          var humanName: String? { __data["humanName"] }
          var favoriteToy: String { __data["favoriteToy"] }
          var owner: Owner? { __data["owner"] }

          struct Fragments: FragmentContainer {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            var heightInMeters: HeightInMeters { _toFragment() }
            var petDetails: PetDetails { _toFragment() }
          }

          /// AllAnimal.AsPet.AsWarmBlooded.Height
          ///
          /// Parent Type: `Height`
          struct Height: TestSchema.SelectionSet {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

            var feet: Int { __data["feet"] }
            var inches: Int? { __data["inches"] }
            var relativeSize: GraphQLEnum<TestSchema.RelativeSize>? { __data["relativeSize"] }
            var centimeters: Double? { __data["centimeters"] }
            var meters: Int { __data["meters"] }
          }

          typealias Owner = PetDetails.Owner
        }
      }

      /// AllAnimal.AsCatIfGetCat
      ///
      /// Parent Type: `Cat`
      struct AsCatIfGetCat: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Cat }
        static var __selections: [Apollo.Selection] { [
          .field("isJellicle", Bool.self),
        ] }

        var isJellicle: Bool { __data["isJellicle"] }
        var height: Height { __data["height"] }
        var species: String? { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var bodyTemperature: Int { __data["bodyTemperature"] }
        var humanName: String? { __data["humanName"] }
        var favoriteToy: String { __data["favoriteToy"] }
        var owner: Owner? { __data["owner"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var heightInMeters: HeightInMeters { _toFragment() }
          var petDetails: PetDetails { _toFragment() }
          var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
        }

        /// AllAnimal.AsCatIfGetCat.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var relativeSize: GraphQLEnum<TestSchema.RelativeSize>? { __data["relativeSize"] }
          var centimeters: Double? { __data["centimeters"] }
          var meters: Int { __data["meters"] }
        }

        typealias Owner = PetDetails.Owner
      }

      /// AllAnimal.AsClassroomPet
      ///
      /// Parent Type: `ClassroomPet`
      struct AsClassroomPet: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Unions.ClassroomPet }
        static var __selections: [Apollo.Selection] { [
          .inlineFragment(AsBird.self),
        ] }

        var height: Height { __data["height"] }
        var species: String? { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }

        var asBird: AsBird? { _asInlineFragment() }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var heightInMeters: HeightInMeters? { _toFragment() }
        }

        /// AllAnimal.AsClassroomPet.AsBird
        ///
        /// Parent Type: `Bird`
        struct AsBird: TestSchema.InlineFragment {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          typealias RootEntityType = AllAnimalsIncludeSkipQuery.Data.AllAnimal
          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Bird }
          static var __selections: [Apollo.Selection] { [
            .field("wingspan", Double.self),
          ] }

          var wingspan: Double { __data["wingspan"] }
          var height: Height { __data["height"] }
          var species: String? { __data["species"] }
          var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
          var predators: [Predator] { __data["predators"] }
          var bodyTemperature: Int { __data["bodyTemperature"] }
          var humanName: String? { __data["humanName"] }
          var favoriteToy: String { __data["favoriteToy"] }
          var owner: Owner? { __data["owner"] }

          struct Fragments: FragmentContainer {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            var heightInMeters: HeightInMeters { _toFragment() }
            var petDetails: PetDetails { _toFragment() }
            var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          }

          /// AllAnimal.AsClassroomPet.AsBird.Height
          ///
          /// Parent Type: `Height`
          struct Height: TestSchema.SelectionSet {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

            var feet: Int { __data["feet"] }
            var inches: Int? { __data["inches"] }
            var relativeSize: GraphQLEnum<TestSchema.RelativeSize>? { __data["relativeSize"] }
            var centimeters: Double? { __data["centimeters"] }
            var meters: Int { __data["meters"] }
          }

          typealias Owner = PetDetails.Owner
        }
      }
    }
  }
}
